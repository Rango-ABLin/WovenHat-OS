#!/usr/bin/env python3
"""Extract the real EFI/BOOT/BOOTX64.EFI payload from a file-backed UEFI image.

This is a read-only artifact-ingress boundary.  It parses the first GPT EFI
System Partition and its FAT directory tree without mounting the image.  Device
paths, symlinks, protected names, and oversized files are rejected.  The
resulting regular file can be passed to the disposable installer transaction's
``--artifact`` option; this tool never writes a disk or claims physical
installation support.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
from typing import Iterator


SECTOR = 512
MAX_ARTIFACT_BYTES = 4 * 1024 * 1024
GPT_ESP_GUID = bytes.fromhex("28732ac11ff8d211ba4b00a0c93ec93b")
PROTECTED_RE = re.compile(r"(?i)(removable|usb|backup|external|physicaldrive|/dev/|\\\\[.?]\\)")


class ExtractError(ValueError):
    """Fail-closed extraction error."""


def _regular(path: Path, label: str) -> Path:
    try:
        stat = path.lstat()
    except OSError as exc:
        raise ExtractError(f"{label} is not readable") from exc
    if path.is_symlink() or not path.is_file():
        raise ExtractError(f"{label} must be a regular non-symlink file")
    if PROTECTED_RE.search(str(path)):
        raise ExtractError(f"{label} path is protected or device-like")
    if stat.st_size == 0 or stat.st_size % SECTOR:
        raise ExtractError(f"{label} must be a non-empty sector-aligned file")
    return path.resolve()


def _esp_bounds(data: bytes) -> tuple[int, int]:
    if len(data) < 2 * SECTOR or data[SECTOR : SECTOR + 8] != b"EFI PART":
        raise ExtractError("image lacks a primary GPT header")
    entry_lba, count, entry_size = struct.unpack_from("<QII", data, SECTOR + 72)
    if count == 0 or count > 128 or entry_size != 128:
        raise ExtractError("GPT partition-entry geometry is unsupported")
    start = entry_lba * SECTOR
    end = start + count * entry_size
    if end > len(data):
        raise ExtractError("GPT partition entries exceed image bounds")
    for offset in range(start, end, entry_size):
        entry = data[offset : offset + entry_size]
        if entry[:16] != GPT_ESP_GUID:
            continue
        first, last = struct.unpack_from("<QQ", entry, 32)
        if first > last or (last + 1) * SECTOR > len(data):
            raise ExtractError("EFI System Partition exceeds image bounds")
        return first * SECTOR, (last - first + 1) * SECTOR
    raise ExtractError("image has no EFI System Partition")


class FatView:
    def __init__(self, esp: bytes):
        if len(esp) < SECTOR or esp[510:512] != b"\x55\xaa":
            raise ExtractError("EFI System Partition lacks a FAT boot signature")
        self.esp = esp
        self.bytes_per_sector, self.sectors_per_cluster = struct.unpack_from("<HB", esp, 11)
        self.reserved, self.fat_count = struct.unpack_from("<HB", esp, 14)
        self.root_entries = struct.unpack_from("<H", esp, 17)[0]
        self.total16 = struct.unpack_from("<H", esp, 19)[0]
        self.sectors_per_fat16 = struct.unpack_from("<H", esp, 22)[0]
        total32 = struct.unpack_from("<I", esp, 32)[0]
        sectors_per_fat32 = struct.unpack_from("<I", esp, 36)[0]
        self.total_sectors = self.total16 or total32
        self.sectors_per_fat = self.sectors_per_fat16 or sectors_per_fat32
        if self.bytes_per_sector != SECTOR or self.sectors_per_cluster == 0 or self.total_sectors == 0:
            raise ExtractError("unsupported FAT geometry")
        if self.reserved == 0 or self.fat_count == 0 or self.sectors_per_fat == 0:
            raise ExtractError("incomplete FAT geometry")
        root_bytes = ((self.root_entries * 32 + SECTOR - 1) // SECTOR)
        self.data_start = (self.reserved + self.fat_count * self.sectors_per_fat + root_bytes) * SECTOR
        data_sectors = self.total_sectors - (self.data_start // SECTOR)
        self.cluster_count = data_sectors // self.sectors_per_cluster
        if self.data_start > len(esp) or self.data_start + data_sectors * SECTOR > len(esp):
            raise ExtractError("FAT data exceeds ESP bounds")
        self.fat32 = self.root_entries == 0
        self.root_cluster = struct.unpack_from("<I", esp, 44)[0] if self.fat32 else 0

    def _next(self, cluster: int) -> int:
        fat_offset = self.reserved * SECTOR + cluster * (4 if self.fat32 else 2)
        if fat_offset + (4 if self.fat32 else 2) > len(self.esp):
            raise ExtractError("FAT entry exceeds ESP bounds")
        value = struct.unpack_from("<I" if self.fat32 else "<H", self.esp, fat_offset)[0]
        return value & 0x0FFFFFFF if self.fat32 else value

    def _cluster(self, cluster: int) -> bytes:
        if cluster < 2 or cluster >= self.cluster_count + 2:
            raise ExtractError("FAT cluster is out of bounds")
        offset = self.data_start + (cluster - 2) * self.sectors_per_cluster * SECTOR
        size = self.sectors_per_cluster * SECTOR
        return self.esp[offset : offset + size]

    def _chain(self, first: int) -> bytes:
        chunks = []
        cluster = first
        seen: set[int] = set()
        while cluster < (0x0FFFFFF8 if self.fat32 else 0xFFF8):
            if cluster in seen:
                raise ExtractError("FAT cluster chain loops")
            seen.add(cluster)
            chunks.append(self._cluster(cluster))
            cluster = self._next(cluster)
            if len(seen) > self.cluster_count:
                raise ExtractError("FAT cluster chain is too long")
        return b"".join(chunks)

    def _root(self) -> bytes:
        if self.fat32:
            return self._chain(self.root_cluster)
        offset = (self.reserved + self.fat_count * self.sectors_per_fat) * SECTOR
        size = self.root_entries * 32
        return self.esp[offset : offset + size]

    def _entries(self, directory: bytes) -> Iterator[tuple[str, bool, int, int]]:
        long_name: list[str] = []
        for offset in range(0, len(directory) - 31, 32):
            entry = directory[offset : offset + 32]
            if entry[0] == 0:
                break
            if entry[0] == 0xE5:
                long_name.clear()
                continue
            attributes = entry[11]
            if attributes == 0x0F:
                pieces = []
                for start, length in ((1, 10), (14, 12), (28, 4)):
                    piece = entry[start : start + length].decode("utf-16le", "ignore")
                    pieces.append(piece.replace("\xff", "").replace("\uffff", "").split("\0", 1)[0])
                long_name.insert(0, "".join(pieces))
                continue
            if attributes & 0x08:
                long_name.clear()
                continue
            base = entry[:8].decode("ascii", "ignore").replace("\xff", "").rstrip()
            extension = entry[8:11].decode("ascii", "ignore").replace("\xff", "").rstrip()
            short_name = base + (("." + extension) if extension else "")
            name = "".join(long_name) or short_name
            long_name.clear()
            cluster = struct.unpack_from("<H", entry, 26)[0] | (struct.unpack_from("<H", entry, 20)[0] << 16)
            yield name, bool(attributes & 0x10), cluster, struct.unpack_from("<I", entry, 28)[0]

    def find(self, components: tuple[str, ...]) -> bytes:
        directory = self._root()
        for index, component in enumerate(components):
            wanted = component.casefold()
            match = next((item for item in self._entries(directory) if item[0].casefold() == wanted), None)
            if match is None:
                raise ExtractError(f"FAT path not found: {'/'.join(components)}")
            name, is_dir, cluster, size = match
            if index + 1 < len(components):
                if not is_dir:
                    raise ExtractError(f"FAT path component is not a directory: {name}")
                directory = self._chain(cluster)
            else:
                if is_dir or size <= 0 or size > MAX_ARTIFACT_BYTES:
                    raise ExtractError("EFI artifact size is outside the bounded range")
                return self._chain(cluster)[:size]
        raise ExtractError("EFI artifact path is empty")


def extract(image: Path, output: Path) -> dict[str, object]:
    source = _regular(image, "source image")
    if output.exists() and output.is_symlink():
        raise ExtractError("output must not be a symlink")
    if PROTECTED_RE.search(str(output)):
        raise ExtractError("output path is protected or device-like")
    data = source.read_bytes()
    offset, length = _esp_bounds(data)
    artifact = FatView(data[offset : offset + length]).find(("EFI", "BOOT", "BOOTX64.EFI"))
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(artifact)
    return {"valid": True, "path": "EFI/BOOT/BOOTX64.EFI", "artifact_bytes": len(artifact), "artifact_sha256": hashlib.sha256(artifact).hexdigest(), "physical_access": "none"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-image", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = extract(args.source_image, args.output)
    except (ExtractError, OSError) as exc:
        print(json.dumps({"valid": False, "error": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
