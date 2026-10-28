#!/usr/bin/env python3
"""Format and verify one disposable layout data region as bounded FAT32 metadata.

This is a test-harness boundary only.  It accepts only an explicitly created
regular file image, consumes the checksummed metadata emitted by
build-file-image-layout.py, and writes only the selected data partition.  It
never opens a device, mounts a filesystem, invokes a formatter, or installs a
bootloader.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import struct
from typing import Any


SECTOR = 512
FAT32_RESERVED = 32
FAT_COUNT = 2
FSINFO_SECTOR = 1
BACKUP_BOOT_SECTOR = 6
BACKUP_FSINFO_SECTOR = 7
ROOT_CLUSTER = 2
SETTINGS_CLUSTER = 3
SETTINGS_ENTRY_NAME = b"INSTALL JSN"
MAX_SETTINGS_BYTES = SECTOR
FAT32_EOC = 0x0FFFFFFF
FAT32_BAD = 0x0FFFFFF7
VOLUME_LABEL = b"WOVENHAT   "
FAT_TYPE = b"FAT32   "
MAX_REGION_BYTES = 128 * 1024 * 1024
DEVICE_RE = re.compile(r"(?i)(?:^|[/\\])dev[/\\]|(?:^|[/\\])physicaldrive\d*$|\\\\[.?]\\physicaldrive")
PROTECTED_RE = re.compile(r"(?i)(system|boot|removable|usb|backup|external|physicaldrive|/dev/|\\\\[.?]\\)")


class FormatError(ValueError):
    """A fail-closed disposable filesystem formatting error."""


def _canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode("utf-8")


def _load_layout_module() -> Any:
    source = Path(__file__).with_name("build-file-image-layout.py")
    spec = importlib.util.spec_from_file_location("file_image_layout", source)
    if spec is None or spec.loader is None:
        raise FormatError("layout builder is unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _image(path: Path, harness_created: bool) -> Path:
    if DEVICE_RE.search(str(path)) or PROTECTED_RE.search(str(path)):
        raise FormatError("physical or protected image paths are forbidden")
    try:
        stat = path.lstat()
    except OSError as exc:
        raise FormatError("image must already exist as an explicit test file") from exc
    if path.is_symlink() or not path.is_file():
        raise FormatError("image must be an existing regular non-symlink file")
    if not harness_created:
        raise FormatError("image requires explicit test-harness creation authorization")
    if stat.st_size < 32 * 1024 * 1024 or stat.st_size > MAX_REGION_BYTES or stat.st_size % SECTOR:
        raise FormatError("image size is outside the bounded aligned test range")
    return path.absolute()


def _read_layout(image: Path, plan: Path) -> tuple[Any, dict[str, Any], dict[str, Any]]:
    layout = _load_layout_module()
    try:
        result = layout.verify(image, plan)
    except (layout.LayoutError, OSError) as exc:
        raise FormatError(str(exc)) from exc
    with image.open("rb") as stream:
        stream.seek(layout.FIRST_USABLE * SECTOR)
        marker = stream.read(len(layout.METADATA_MAGIC))
        if marker != layout.METADATA_MAGIC:
            raise FormatError("layout metadata is missing")
        length = int.from_bytes(stream.read(4), "little")
        raw = stream.read(length)
    try:
        metadata = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise FormatError("layout metadata is invalid") from exc
    supplied = metadata.pop("sha256", None)
    if supplied != hashlib.sha256(layout._canonical(metadata)).hexdigest():
        raise FormatError("layout metadata checksum mismatch")
    regions = [item for item in metadata.get("partitions", []) if item.get("type") == "data"]
    if len(regions) != 1:
        raise FormatError("layout must contain exactly one data region")
    region = regions[0]
    if result.get("valid") is not True:
        raise FormatError("layout verification failed")
    return layout, metadata, region


def _region_bounds(image: Path, region: dict[str, Any]) -> tuple[int, int, int]:
    first = region.get("first_lba")
    last = region.get("last_lba")
    if not isinstance(first, int) or not isinstance(last, int) or first <= 0 or last < first:
        raise FormatError("data region bounds are invalid")
    offset = first * SECTOR
    size = (last - first + 1) * SECTOR
    if size != region.get("bytes") or size > MAX_REGION_BYTES or offset + size > image.stat().st_size:
        raise FormatError("data region is outside the image")
    return offset, size, last - first + 1


def _parameters(region_sectors: int) -> tuple[int, int, int]:
    if region_sectors <= FAT32_RESERVED + FAT_COUNT:
        raise FormatError("data region is too small for FAT32 metadata")
    clusters = region_sectors - FAT32_RESERVED
    while True:
        fat_sectors = (clusters + 2) * 4
        fat_sectors = (fat_sectors + SECTOR - 1) // SECTOR
        next_clusters = region_sectors - FAT32_RESERVED - FAT_COUNT * fat_sectors
        if next_clusters == clusters:
            break
        clusters = next_clusters
        if clusters <= 0:
            raise FormatError("data region cannot contain a FAT32 data area")
    if clusters < 65525:
        raise FormatError("data region is below the FAT32 minimum cluster count")
    return fat_sectors, clusters, 1


def _boot_sector(region_sectors: int, fat_sectors: int, serial: int) -> bytes:
    raw = bytearray(SECTOR)
    raw[0:3] = b"\xEB\x58\x90"
    raw[3:11] = b"WOVENOS "
    struct.pack_into("<H", raw, 11, SECTOR)
    raw[13] = 1
    struct.pack_into("<H", raw, 14, FAT32_RESERVED)
    raw[16] = FAT_COUNT
    struct.pack_into("<H", raw, 17, 0)
    struct.pack_into("<H", raw, 19, 0)
    raw[21] = 0xF8
    struct.pack_into("<H", raw, 22, 0)
    struct.pack_into("<H", raw, 24, 63)
    struct.pack_into("<H", raw, 26, 255)
    struct.pack_into("<I", raw, 28, 0)
    struct.pack_into("<I", raw, 32, region_sectors)
    struct.pack_into("<I", raw, 36, fat_sectors)
    struct.pack_into("<H", raw, 40, 0)
    struct.pack_into("<H", raw, 42, 0)
    struct.pack_into("<I", raw, 44, ROOT_CLUSTER)
    struct.pack_into("<H", raw, 48, FSINFO_SECTOR)
    struct.pack_into("<H", raw, 50, BACKUP_BOOT_SECTOR)
    raw[64] = 0x80
    raw[66] = 0x29
    struct.pack_into("<I", raw, 67, serial)
    raw[71:82] = VOLUME_LABEL
    raw[82:90] = FAT_TYPE
    raw[510:512] = b"\x55\xAA"
    return bytes(raw)


def _fsinfo(clusters: int) -> bytes:
    raw = bytearray(SECTOR)
    struct.pack_into("<I", raw, 0, 0x41615252)
    struct.pack_into("<I", raw, 484, 0x61417272)
    struct.pack_into("<I", raw, 488, max(0, clusters - 1))
    struct.pack_into("<I", raw, 492, ROOT_CLUSTER + 1)
    struct.pack_into("<I", raw, 508, 0xAA550000)
    return bytes(raw)


def _fat(fat_sectors: int, extra: dict[int, int] | None = None) -> bytes:
    raw = bytearray(fat_sectors * SECTOR)
    struct.pack_into("<I", raw, 0, 0x0FFFFFF8)
    struct.pack_into("<I", raw, 4, FAT32_EOC)
    struct.pack_into("<I", raw, 8, FAT32_EOC)
    for cluster, value in (extra or {}).items():
        struct.pack_into("<I", raw, cluster * 4, value & 0x0FFFFFFF)
    return bytes(raw)


def _settings_entry(cluster: int, size: int) -> bytes:
    raw = bytearray(32)
    raw[0:11] = SETTINGS_ENTRY_NAME
    raw[11] = 0x20
    struct.pack_into("<H", raw, 20, (cluster >> 16) & 0xFFFF)
    struct.pack_into("<H", raw, 26, cluster & 0xFFFF)
    struct.pack_into("<I", raw, 28, size)
    return bytes(raw)


def _volume(image: Path, metadata: dict[str, Any], region: dict[str, Any], settings: dict[str, Any] | None = None) -> bytes:
    offset, size, region_sectors = _region_bounds(image, region)
    del offset
    fat_sectors, clusters, _ = _parameters(region_sectors)
    serial_bytes = hashlib.sha256((metadata["disk_guid"] + ":" + str(region["first_lba"]) + ":" + str(region["last_lba"])).encode()).digest()
    serial = int.from_bytes(serial_bytes[:4], "little")
    raw = bytearray(size)
    boot = _boot_sector(region_sectors, fat_sectors, serial)
    info = _fsinfo(clusters)
    settings_payload: bytes | None = None
    extra_fat: dict[int, int] = {}
    if settings is not None:
        settings_payload = _canonical(settings)
        if len(settings_payload) > MAX_SETTINGS_BYTES:
            raise FormatError("settings payload exceeds the bounded single-cluster size")
        extra_fat[SETTINGS_CLUSTER] = FAT32_EOC
    fat = _fat(fat_sectors, extra_fat or None)
    raw[0:SECTOR] = boot
    raw[SECTOR:2 * SECTOR] = info
    raw[BACKUP_BOOT_SECTOR * SECTOR:(BACKUP_BOOT_SECTOR + 1) * SECTOR] = boot
    raw[BACKUP_FSINFO_SECTOR * SECTOR:(BACKUP_FSINFO_SECTOR + 1) * SECTOR] = info
    fat_start = FAT32_RESERVED * SECTOR
    raw[fat_start:fat_start + len(fat)] = fat
    raw[fat_start + len(fat):fat_start + 2 * len(fat)] = fat
    if settings_payload is not None:
        data_start = fat_start + 2 * len(fat)
        root_entry = _settings_entry(SETTINGS_CLUSTER, len(settings_payload))
        raw[data_start:data_start + len(root_entry)] = root_entry
        settings_offset = data_start + (SETTINGS_CLUSTER - ROOT_CLUSTER) * SECTOR
        raw[settings_offset:settings_offset + len(settings_payload)] = settings_payload
    return bytes(raw)


def read_installed_settings(image: Path, plan: Path, *, harness_created: bool = True) -> dict[str, Any] | None:
    """Read back the settings file written into the root directory, if any."""
    image = _image(image, harness_created)
    _, _, region = _read_layout(image, plan)
    offset, size, region_sectors = _region_bounds(image, region)
    fat_sectors, _, _ = _parameters(region_sectors)
    fat_start = FAT32_RESERVED * SECTOR
    data_start = fat_start + 2 * fat_sectors * SECTOR
    with image.open("rb") as stream:
        stream.seek(offset + data_start)
        root_sector = stream.read(SECTOR)
        entry = root_sector[:32]
        if entry == b"\0" * 32:
            return None
        if entry[0:11] != SETTINGS_ENTRY_NAME or entry[11] != 0x20:
            raise FormatError("root directory entry is not the expected settings file")
        cluster = struct.unpack_from("<H", entry, 26)[0] | (struct.unpack_from("<H", entry, 20)[0] << 16)
        file_size = struct.unpack_from("<I", entry, 28)[0]
        if cluster != SETTINGS_CLUSTER or file_size <= 0 or file_size > MAX_SETTINGS_BYTES:
            raise FormatError("settings directory entry is invalid")
        stream.seek(offset + data_start + (cluster - ROOT_CLUSTER) * SECTOR)
        payload = stream.read(file_size)
    if len(payload) != file_size:
        raise FormatError("settings file is truncated")
    try:
        return json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise FormatError("settings file content is invalid") from exc


def verify(image: Path, plan: Path, *, harness_created: bool = True, settings: dict[str, Any] | None = None) -> dict[str, Any]:
    image = _image(image, harness_created)
    layout, metadata, region = _read_layout(image, plan)
    offset, size, region_sectors = _region_bounds(image, region)
    with image.open("rb") as stream:
        stream.seek(offset)
        raw = stream.read(size)
    if len(raw) != size or raw[510:512] != b"\x55\xAA":
        raise FormatError("FAT32 boot signature is invalid")
    if raw[3:11] != b"WOVENOS " or raw[71:82] != VOLUME_LABEL or raw[82:90] != FAT_TYPE:
        raise FormatError("FAT32 identity metadata is invalid")
    if struct.unpack_from("<H", raw, 11)[0] != SECTOR or raw[13] != 1:
        raise FormatError("FAT32 sector geometry is invalid")
    reserved = struct.unpack_from("<H", raw, 14)[0]
    fats = raw[16]
    fat_sectors = struct.unpack_from("<I", raw, 36)[0]
    if reserved != FAT32_RESERVED or fats != FAT_COUNT or struct.unpack_from("<I", raw, 32)[0] != region_sectors:
        raise FormatError("FAT32 bounds metadata is invalid")
    if struct.unpack_from("<H", raw, 48)[0] != FSINFO_SECTOR or struct.unpack_from("<H", raw, 50)[0] != BACKUP_BOOT_SECTOR:
        raise FormatError("FAT32 auxiliary-sector pointers are invalid")
    if raw[SECTOR:2 * SECTOR] != raw[BACKUP_FSINFO_SECTOR * SECTOR:(BACKUP_FSINFO_SECTOR + 1) * SECTOR]:
        raise FormatError("FSInfo backup does not match")
    if raw[:SECTOR] != raw[BACKUP_BOOT_SECTOR * SECTOR:(BACKUP_BOOT_SECTOR + 1) * SECTOR]:
        raise FormatError("boot-sector backup does not match")
    fat_start = reserved * SECTOR
    fat_bytes = fat_sectors * SECTOR
    if fat_start + FAT_COUNT * fat_bytes > size:
        raise FormatError("FAT bounds exceed data region")
    if raw[fat_start:fat_start + fat_bytes] != raw[fat_start + fat_bytes:fat_start + 2 * fat_bytes]:
        raise FormatError("FAT copies do not match")
    if struct.unpack_from("<I", raw, fat_start)[0] & 0x0FFFFFF8 != 0x0FFFFFF8:
        raise FormatError("FAT reserved entry is invalid")
    if struct.unpack_from("<I", raw, fat_start + 8)[0] & 0x0FFFFFFF != FAT32_EOC:
        raise FormatError("root cluster entry is invalid")
    expected = _volume(image, metadata, region, settings)
    if raw != expected:
        raise FormatError("filesystem metadata is not deterministic")
    return {"valid": True, "filesystem": "fat32-compatible", "label": VOLUME_LABEL.decode().rstrip(), "first_lba": region["first_lba"], "last_lba": region["last_lba"], "fat_sectors": fat_sectors, "image_sha256": hashlib.sha256(raw).hexdigest(), "physical_access": "none", "layout_module": str(layout.__name__), "settings_embedded": settings is not None}


def format_image(image: Path, plan: Path, *, harness_created: bool = False, inject_failure: str | None = None, settings: dict[str, Any] | None = None) -> dict[str, Any]:
    image = _image(image, harness_created)
    layout, metadata, region = _read_layout(image, plan)
    offset, size, _ = _region_bounds(image, region)
    with image.open("rb") as stream:
        stream.seek(offset)
        before = stream.read(size)
    formatted = _volume(image, metadata, region, settings)
    if inject_failure == "before-write":
        raise FormatError("injected failure before data-region write")
    try:
        with image.open("r+b") as stream:
            stream.seek(offset)
            stream.write(formatted)
            stream.flush()
            os.fsync(stream.fileno())
            if inject_failure == "after-write":
                raise FormatError("injected failure after data-region write")
            verify(image, plan, harness_created=True, settings=settings)
            if inject_failure == "after-verify":
                raise FormatError("injected failure after verification")
    except (OSError, FormatError):
        with image.open("r+b") as stream:
            stream.seek(offset)
            stream.write(before)
            stream.flush()
            os.fsync(stream.fileno())
        raise
    del layout
    return verify(image, plan, harness_created=True, settings=settings)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True, type=Path)
    parser.add_argument("--validated-plan", required=True, type=Path)
    parser.add_argument("--test-harness-created", action="store_true")
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--inject-failure", choices=("before-write", "after-write", "after-verify"))
    parser.add_argument("--settings-file", type=Path, help="optional JSON settings to embed as a single root-directory file")
    args = parser.parse_args()
    try:
        settings = json.loads(args.settings_file.read_text(encoding="utf-8")) if args.settings_file else None
        result = verify(args.image, args.validated_plan, harness_created=True, settings=settings) if args.verify else format_image(args.image, args.validated_plan, harness_created=args.test_harness_created, inject_failure=args.inject_failure, settings=settings)
    except (FormatError, OSError, json.JSONDecodeError) as exc:
        print(json.dumps({"valid": False, "error": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
