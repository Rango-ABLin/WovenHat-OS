#!/usr/bin/env python3
"""Format the disposable GPT data region with the exact WovenFS v1 layout."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import struct
from pathlib import Path
from typing import Any

SECTOR = 512
JOURNAL_BLOCKS = 34
INODE_COUNT = 256
INODE_SIZE = 128
INODES_PER_BLOCK = 4
DIRECT_BLOCKS = 8
KIND_FILE = 1
KIND_DIR = 2
MAX_FILE = DIRECT_BLOCKS * SECTOR
MAGIC = b"WOVENFS1"


class FormatError(ValueError):
    pass


def _layout():
    path = Path(__file__).with_name("build-file-image-layout.py")
    spec = importlib.util.spec_from_file_location("wfs_layout", path)
    if spec is None or spec.loader is None:
        raise FormatError("layout builder unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _region(image: Path, plan: Path):
    layout = _layout()
    result = layout.verify(image, plan)
    if result.get("valid") is not True:
        raise FormatError("layout verification failed")
    metadata = None
    with image.open("rb") as stream:
        stream.seek(layout.FIRST_USABLE * SECTOR)
        marker = stream.read(len(layout.METADATA_MAGIC))
        if marker != layout.METADATA_MAGIC:
            raise FormatError("layout metadata missing")
        length = int.from_bytes(stream.read(4), "little")
        metadata = json.loads(stream.read(length).decode("utf-8"))
    regions = [item for item in metadata["partitions"] if item.get("type") == "data"]
    if len(regions) != 1:
        raise FormatError("layout must contain one data region")
    region = regions[0]
    first, last = region["first_lba"], region["last_lba"]
    size = (last - first + 1) * SECTOR
    if size != region["bytes"] or first <= 0 or last < first:
        raise FormatError("invalid data region")
    return layout, region, first * SECTOR, size, last - first + 1


def _fnv(data: bytes) -> int:
    value = 0xCBF29CE484222325
    for byte in data:
        value = ((value ^ byte) * 0x00000100000001B3) & 0xFFFFFFFFFFFFFFFF
    return value


def _super(total: int, sequence: int = 1) -> bytes:
    bitmap_blocks = (total + 4095) // 4096
    inode_start = 2 + JOURNAL_BLOCKS + bitmap_blocks
    data_start = inode_start + (INODE_COUNT + 3) // 4
    raw = bytearray(SECTOR)
    raw[0:8] = MAGIC
    struct.pack_into("<H", raw, 8, 1)
    fields = (sequence, total, 2, JOURNAL_BLOCKS, 2 + JOURNAL_BLOCKS,
              bitmap_blocks, inode_start, INODE_COUNT, data_start, 1)
    struct.pack_into("<Q", raw, 12, fields[0])
    struct.pack_into("<Q", raw, 20, fields[1])
    struct.pack_into("<Q", raw, 28, fields[2])
    struct.pack_into("<I", raw, 36, fields[3])
    struct.pack_into("<Q", raw, 40, fields[4])
    struct.pack_into("<I", raw, 48, fields[5])
    struct.pack_into("<Q", raw, 52, fields[6])
    struct.pack_into("<I", raw, 60, fields[7])
    struct.pack_into("<Q", raw, 64, fields[8])
    struct.pack_into("<Q", raw, 72, fields[9])
    struct.pack_into("<Q", raw, 80, _fnv(raw[:80]))
    return bytes(raw)


def _inode(mode: int, links: int, size: int, kind: int, direct: list[int], created: int = 0, data_checksum: int = 0) -> bytes:
    raw = bytearray(INODE_SIZE)
    struct.pack_into("<IIQQQQ", raw, 0, mode, links, size, created, created, data_checksum)
    for index, block in enumerate(direct[:DIRECT_BLOCKS]):
        struct.pack_into("<Q", raw, 40 + index * 8, block)
    struct.pack_into("<H", raw, 104, kind)
    struct.pack_into("<Q", raw, 120, _fnv(raw[:120]))
    return bytes(raw)


def _entry(inode: int, kind: int, name: bytes) -> bytes:
    raw = bytearray(64)
    struct.pack_into("<QHH", raw, 0, inode, len(name), kind)
    raw[16:16 + len(name)] = name
    return bytes(raw)


def _build(size: int, settings: dict[str, Any] | None) -> bytes:
    total = size // SECTOR
    if total < 16:
        raise FormatError("WovenFS region too small")
    sb = _super(total)
    data_start = struct.unpack_from("<Q", sb, 64)[0]
    bitmap_start = struct.unpack_from("<Q", sb, 40)[0]
    bitmap_blocks = struct.unpack_from("<I", sb, 48)[0]
    inode_start = struct.unpack_from("<Q", sb, 52)[0]
    raw = bytearray(size)
    raw[0:SECTOR] = sb
    bitmap = bytearray(bitmap_blocks * SECTOR)
    for block in range(data_start + 1):
        bitmap[block // 8] |= 1 << (block % 8)
    raw[bitmap_start * SECTOR:(bitmap_start + bitmap_blocks) * SECTOR] = bitmap
    root_block = data_start
    root = bytearray(SECTOR)
    inode_count = 1
    if settings is not None:
        payload = (json.dumps(settings, sort_keys=True, separators=(",", ":")) + "\n").encode()
        if len(payload) > MAX_FILE:
            raise FormatError("settings exceed WovenFS bounded file size")
        file_inode = 2
        file_block = data_start + 1
        bitmap[file_block // 8] |= 1 << (file_block % 8)
        raw[bitmap_start * SECTOR:(bitmap_start + bitmap_blocks) * SECTOR] = bitmap
        root[:64] = _entry(file_inode, KIND_FILE, b"INSTALL.JSON")
        raw[file_block * SECTOR:file_block * SECTOR + len(payload)] = payload
        raw[(inode_start + 0) * SECTOR:(inode_start + 1) * SECTOR] = _inode(0o040755, 2, SECTOR, KIND_DIR, [root_block])
        raw[inode_start * SECTOR + INODE_SIZE:inode_start * SECTOR + 2 * INODE_SIZE] = _inode(0o100600, 1, len(payload), KIND_FILE, [file_block], data_checksum=_fnv(payload))
        inode_count = 2
    else:
        raw[inode_start * SECTOR:inode_start * SECTOR + INODE_SIZE] = _inode(0o040755, 2, SECTOR, KIND_DIR, [root_block])
    raw[root_block * SECTOR:(root_block + 1) * SECTOR] = root
    if settings is not None:
        raw[file_block * SECTOR:file_block * SECTOR + len(payload)] = payload
    return bytes(raw).ljust(size, b"\0")


def format_image(image: Path, plan: Path, *, harness_created: bool = False, settings: dict[str, Any] | None = None, **_: Any) -> dict[str, Any]:
    if not harness_created or image.is_symlink() or not image.is_file():
        raise FormatError("explicit disposable regular file is required")
    _, region, offset, size, _ = _region(image, plan)
    data = _build(size, settings)
    with image.open("r+b") as stream:
        stream.seek(offset)
        stream.write(data)
        stream.flush()
    return {"valid": True, "filesystem": "wovenfs", "first_lba": region["first_lba"], "last_lba": region["last_lba"], "image_sha256": hashlib.sha256(data).hexdigest(), "physical_access": "none", "settings_embedded": settings is not None}


def verify(image: Path, plan: Path, *, harness_created: bool = True, settings: dict[str, Any] | None = None, **_: Any) -> dict[str, Any]:
    _, region, offset, size, _ = _region(image, plan)
    with image.open("rb") as stream:
        stream.seek(offset)
        actual = stream.read(size)
    expected = _build(size, settings)
    if actual != expected:
        raise FormatError("WovenFS metadata is not deterministic")
    return {"valid": True, "filesystem": "wovenfs", "first_lba": region["first_lba"], "last_lba": region["last_lba"], "image_sha256": hashlib.sha256(actual).hexdigest(), "physical_access": "none", "settings_embedded": settings is not None}


def read_installed_settings(image: Path, plan: Path, *, harness_created: bool = True) -> dict[str, Any] | None:
    _, _, offset, size, _ = _region(image, plan)
    with image.open("rb") as stream:
        stream.seek(offset)
        raw = stream.read(size)
    inode_start = struct.unpack_from("<Q", raw, 52)[0] * SECTOR
    length = struct.unpack_from("<Q", raw, inode_start + INODE_SIZE + 8)[0]
    if length == 0:
        return None
    block = struct.unpack_from("<Q", raw, inode_start + INODE_SIZE + 40)[0]
    payload = raw[block * SECTOR:block * SECTOR + length]
    return json.loads(payload.decode())
