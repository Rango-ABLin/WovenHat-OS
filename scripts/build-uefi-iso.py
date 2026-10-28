"""Build a UEFI-bootable ISO9660/El Torito image from a WovenHat disk image.

The source image is treated as immutable.  The first GPT partition whose type
is the standard EFI System Partition type is copied as the El Torito EFI boot
image.  No mounts, device paths, firmware writes, or physical media access are
performed.  The output is written transactionally and is accepted only as a
file-backed artifact until the exact ISO passes the QEMU gate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import struct
import tempfile
from pathlib import Path


SECTOR = 512
ISO_SECTOR = 2048
MAX_EL_TORITO_SECTORS = 0x10000
GPT_HEADER_LBA = 1
GPT_ENTRY_LBA = 2
GPT_ENTRY_SIZE = 128
GPT_ENTRY_COUNT = 128
ESP_GUID = bytes.fromhex("28732ac11ff8d211ba4b00a0c93ec93b")
PROTECTED_RE = re.compile(
    r"(?i)(removable|usb|backup|external|physicaldrive|/dev/|\\\\[.?]\\)"
)


class IsoError(ValueError):
    """Fail-closed ISO construction error."""


def _regular(path: Path, label: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise IsoError(f"{label} must be a regular non-symlink file: {path}")
    if PROTECTED_RE.search(str(path)):
        raise IsoError(f"{label} path is protected or device-like: {path}")


def _read_gpt(path: Path) -> tuple[int, int]:
    _regular(path, "source image")
    size = path.stat().st_size
    if size == 0 or size % SECTOR:
        raise IsoError("source image must be a non-zero 512-byte multiple")
    with path.open("rb") as stream:
        stream.seek(GPT_HEADER_LBA * SECTOR)
        header = stream.read(SECTOR)
        if header[:8] != b"EFI PART":
            raise IsoError("source image lacks a primary GPT header")
        entry_lba, count, entry_size = struct.unpack_from("<QII", header, 72)
        if count == 0 or count > GPT_ENTRY_COUNT or entry_size != GPT_ENTRY_SIZE:
            raise IsoError("GPT partition-entry geometry is unsupported")
        if entry_lba * SECTOR + count * entry_size > size:
            raise IsoError("GPT partition-entry array exceeds source image")
        stream.seek(entry_lba * SECTOR)
        entries = stream.read(count * entry_size)
    for offset in range(0, len(entries), entry_size):
        entry = entries[offset : offset + entry_size]
        if entry[:16] != ESP_GUID:
            continue
        first, last = struct.unpack_from("<QQ", entry, 32)
        if first > last or (last + 1) * SECTOR > size:
            raise IsoError("EFI System Partition is outside the source image")
        return first * SECTOR, (last - first + 1) * SECTOR
    raise IsoError("source image has no EFI System Partition")


def _extract_esp(source: Path) -> bytes:
    offset, length = _read_gpt(source)
    with source.open("rb") as stream:
        stream.seek(offset)
        esp = stream.read(length)
    if len(esp) != length or len(esp) < ISO_SECTOR:
        raise IsoError("EFI System Partition could not be read completely")
    if esp[510:512] != b"\x55\xaa":
        raise IsoError("EFI System Partition lacks a FAT boot signature")
    # The BPB file-system-type label sits at offset 54 for FAT12/16 and at
    # offset 82 for FAT32 (after the FAT32 extended BPB fields).
    fat16 = esp[54:62].rstrip(b" ") == b"FAT16"
    fat32 = esp[82:90].rstrip(b" ") == b"FAT32"
    if not (fat16 or fat32):
        raise IsoError("EFI System Partition is not FAT16 or FAT32")
    boot_name_seen = (b"BOOTX64.EFI" in esp.upper()
                      or b"BOOTX64 EFI" in esp.upper())
    if not boot_name_seen:
        raise IsoError("EFI System Partition does not contain BOOTX64.EFI bytes")
    max_bytes = MAX_EL_TORITO_SECTORS * SECTOR
    if len(esp) <= max_bytes:
        return esp
    # El Torito stores the EFI image size in a 16-bit 512-byte-sector field;
    # zero represents exactly 65536 sectors. A bounded copy is safe only when
    # the discarded tail is all zero and the FAT BPB is reduced accordingly.
    if any(esp[max_bytes:]):
        raise IsoError("EFI System Partition exceeds El Torito limit with non-zero tail")
    bounded = bytearray(esp[:max_bytes])
    total16 = struct.unpack_from("<H", bounded, 19)[0]
    total32 = struct.unpack_from("<I", bounded, 32)[0]
    original_total = total16 or total32
    if original_total > MAX_EL_TORITO_SECTORS:
        struct.pack_into("<H", bounded, 19, 0)
        struct.pack_into("<I", bounded, 32, MAX_EL_TORITO_SECTORS)
    return bytes(bounded)


def _both_endian_u32(value: int) -> bytes:
    return struct.pack("<I", value) + struct.pack(">I", value)


def _dir_record(extent: int, size: int, flags: int, name: bytes) -> bytes:
    record_len = 33 + len(name) + (len(name) % 2 == 0)
    record = bytearray(record_len)
    record[0] = record_len
    record[2:6] = struct.pack("<I", extent)
    record[6:10] = struct.pack(">I", extent)
    record[10:14] = struct.pack("<I", size)
    record[14:18] = struct.pack(">I", size)
    record[25] = flags
    record[28:32] = struct.pack("<HH", 1, 1)
    record[32] = len(name)
    record[33 : 33 + len(name)] = name
    return bytes(record)


def _pvd(volume_id: str, volume_sectors: int, root_extent: int) -> bytes:
    block = bytearray(ISO_SECTOR)
    block[0] = 1
    block[1:6] = b"CD001"
    block[6] = 1
    block[8:40] = volume_id.encode("ascii").ljust(32, b" ")[:32]
    block[80:88] = _both_endian_u32(volume_sectors)
    block[120:124] = struct.pack("<HH", 1, 1)
    block[124:128] = struct.pack(">HH", 1, 1)
    block[128:132] = struct.pack("<H", ISO_SECTOR) + struct.pack(">H", ISO_SECTOR)
    root = _dir_record(root_extent, ISO_SECTOR, 2, b"\x00")
    block[156 : 156 + len(root)] = root
    block[881] = 1
    return bytes(block)


def _boot_record(catalog_lba: int) -> bytes:
    block = bytearray(ISO_SECTOR)
    block[0] = 0
    block[1:6] = b"CD001"
    block[6] = 1
    block[7:39] = b"EL TORITO SPECIFICATION".ljust(32, b" ")
    struct.pack_into("<I", block, 71, catalog_lba)
    return bytes(block)


def _catalog(boot_lba: int, boot_bytes: int) -> bytes:
    block = bytearray(ISO_SECTOR)
    validation = bytearray(32)
    validation[0] = 1
    validation[1] = 0xef
    validation[4:28] = b"WOVENHAT UEFI BOOT CATALOG".ljust(24, b" ")[:24]
    validation[30] = 0x55
    validation[31] = 0xaa
    checksum = (-sum(struct.unpack("<16H", validation))) & 0xFFFF
    struct.pack_into("<H", validation, 28, checksum)
    block[:32] = validation
    entry = bytearray(32)
    entry[0] = 0x91
    entry[1] = 0xef
    struct.pack_into("<H", entry, 2, 1)
    entry[4:28] = b"WOVENHAT UEFI".ljust(24, b" ")
    default = bytearray(32)
    default[0] = 0x88
    default[1] = 0
    boot_sectors = boot_bytes // SECTOR
    if boot_sectors == 0 or boot_sectors > MAX_EL_TORITO_SECTORS:
        raise IsoError("EFI boot image must contain 1-65536 512-byte sectors")
    # El Torito encodes 65536 sectors as zero in the 16-bit count field.
    struct.pack_into("<H", default, 6, 0 if boot_sectors == 0x10000 else boot_sectors)
    struct.pack_into("<I", default, 8, boot_lba)
    block[32:64] = entry
    block[64:96] = default
    return bytes(block)


def build_iso(source: Path, output: Path, volume_id: str = "WOVENHAT") -> dict[str, object]:
    if output.exists() and output.is_symlink():
        raise IsoError("output must not be a symlink")
    if output.resolve() == source.resolve():
        raise IsoError("output must differ from source image")
    if PROTECTED_RE.search(str(output)):
        raise IsoError(f"output path is protected or device-like: {output}")
    if not re.fullmatch(r"[A-Z0-9_]{1,32}", volume_id):
        raise IsoError("volume ID must be 1-32 ASCII A-Z, digits, or underscore")
    esp = _extract_esp(source)
    if len(esp) % ISO_SECTOR:
        esp += b"\0" * (ISO_SECTOR - len(esp) % ISO_SECTOR)

    root_lba, catalog_lba, boot_lba = 19, 19, 20
    # Root directory, descriptor terminator, catalog, and EFI boot image.
    root_data_lba = 19
    root_record = _dir_record(root_data_lba, ISO_SECTOR, 2, b"\x00")
    root_record += _dir_record(root_data_lba, ISO_SECTOR, 2, b"\x01")
    root_record += _dir_record(boot_lba, len(esp), 0, b"WOVENHAT.EFI;1")
    root_data = root_record.ljust(ISO_SECTOR, b"\0")
    catalog_lba = 20
    boot_lba = 21
    total = boot_lba + len(esp) // ISO_SECTOR
    blocks = [b"\0" * ISO_SECTOR] * 16
    blocks += [_pvd(volume_id, total, root_data_lba), _boot_record(catalog_lba)]
    blocks += [bytes([255]) + b"CD001" + bytes([1]) + b"\0" * (ISO_SECTOR - 7)]
    blocks += [root_data, _catalog(boot_lba, len(esp))]
    blocks += [esp[i : i + ISO_SECTOR] for i in range(0, len(esp), ISO_SECTOR)]
    payload = b"".join(blocks)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(prefix=output.name + ".", dir=output.parent, delete=False) as tmp:
        temp = Path(tmp.name)
        tmp.write(payload)
        tmp.flush()
        os.fsync(tmp.fileno())
    os.replace(temp, output)
    return {
        "format": "ISO9660-El-Torito-UEFI",
        "source_image": str(source.resolve()),
        "output": str(output.resolve()),
        "bytes": len(payload),
        "esp_bytes": len(esp),
        "boot_catalog_lba": catalog_lba,
        "efi_boot_image_lba": boot_lba,
        "sha256": hashlib.sha256(payload).hexdigest(),
        "bootability": "requires-exact-artifact-QEMU-gate",
        "physical_disk_access": False,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-image", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--volume-id", default="WOVENHAT")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    try:
        result = build_iso(args.source_image.resolve(), args.output.resolve(), args.volume_id)
    except (OSError, IsoError) as exc:
        parser.error(str(exc))
    print(json.dumps(result, indent=2, sort_keys=True) if args.json else result["output"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
