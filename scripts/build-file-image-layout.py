#!/usr/bin/env python3
"""Build and verify a deterministic GPT-like layout in a disposable file image.

This is a test-harness boundary only.  It never opens a device, mounts a
filesystem, formats media, installs a bootloader, or calls partition APIs.
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
import uuid
import zlib


SECTOR = 512
ALIGN_SECTORS = 2048
ESP_BYTES = 8 * 1024 * 1024
GPT_ENTRIES = 128
GPT_ENTRY_BYTES = 128
GPT_ENTRIES_SECTORS = (GPT_ENTRIES * GPT_ENTRY_BYTES + SECTOR - 1) // SECTOR
FIRST_USABLE = 2 + GPT_ENTRIES_SECTORS
MAX_IMAGE_BYTES = 128 * 1024 * 1024
METADATA_MAGIC = b"WOVEN-LAYOUT-V1\0"
DEVICE_RE = re.compile(r"(?i)(?:^|[/\\])dev[/\\]|(?:^|[/\\])physicaldrive\d*$|\\\\[.?]\\physicaldrive")
PROTECTED_RE = re.compile(r"(?i)(system|boot|removable|usb|backup|external|physicaldrive|/dev/|\\\\[.?]\\)")


class LayoutError(ValueError):
    """A fail-closed disposable-image layout error."""


def _load_plan(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise LayoutError("validated plan must be a regular file")
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise LayoutError("validated plan is not readable JSON") from exc
    if not isinstance(doc, dict) or doc.get("valid") is not True:
        raise LayoutError("plan is not marked valid")
    plan = doc.get("plan")
    if not isinstance(plan, dict) or plan.get("schema") != "woveninstaller-plan-v1":
        raise LayoutError("plan schema is unsupported")
    if plan.get("partition_table") != "gpt" or plan.get("filesystem") not in ("ext4", "fat32", "wovenfs"):
        raise LayoutError("plan is not a GPT file-image plan")
    target = plan.get("target_disk")
    if not isinstance(target, dict) or not isinstance(target.get("path"), str):
        raise LayoutError("plan target identity is incomplete")
    if PROTECTED_RE.search(target["path"]):
        raise LayoutError("plan target identity is protected")
    if not target["path"].startswith("file-image:"):
        raise LayoutError("plan target is not file-backed")
    if not isinstance(target.get("serial"), str) or not target["serial"]:
        raise LayoutError("plan target identity is protected or incomplete")
    parts = plan.get("partitions")
    if not isinstance(parts, list) or not parts:
        raise LayoutError("plan has no data partitions")
    for item in parts:
        if not isinstance(item, dict) or not isinstance(item.get("name"), str) or not isinstance(item.get("size_bytes"), int):
            raise LayoutError("plan contains an invalid partition")
        if item["size_bytes"] <= 0 or item["size_bytes"] % SECTOR:
            raise LayoutError("plan partition is not sector-aligned")
    summary = plan.get("summary")
    if not isinstance(summary, dict) or not isinstance(summary.get("target_bytes"), int):
        raise LayoutError("plan target capacity is missing")
    return plan


def _image(path: Path, harness_created: bool) -> Path:
    if DEVICE_RE.search(str(path)) or PROTECTED_RE.search(str(path)):
        raise LayoutError("physical or protected image paths are forbidden")
    try:
        stat = path.lstat()
    except OSError as exc:
        raise LayoutError("image must already exist as an explicit test file") from exc
    if path.is_symlink() or not path.is_file():
        raise LayoutError("image must be an existing regular non-symlink file")
    if not harness_created:
        raise LayoutError("image requires explicit test-harness creation authorization")
    if stat.st_size < 32 * 1024 * 1024 or stat.st_size > MAX_IMAGE_BYTES or stat.st_size % SECTOR:
        raise LayoutError("image size is outside the bounded aligned test range")
    return path.absolute()


def _align(value: int) -> int:
    return (value + ALIGN_SECTORS - 1) // ALIGN_SECTORS * ALIGN_SECTORS


def _guid(seed: bytes) -> bytes:
    return uuid.UUID(bytes=hashlib.sha256(seed).digest()[:16]).bytes_le


def _header(current: int, backup: int, first: int, last: int, disk_guid: bytes, entries_lba: int, entries_crc: int) -> bytes:
    raw = bytearray(SECTOR)
    raw[:8] = b"EFI PART"
    struct.pack_into("<I I I Q Q Q Q 16s Q I I I", raw, 8, 0x00010000, 92, 0, current, backup, first, last, disk_guid, entries_lba, GPT_ENTRIES, GPT_ENTRY_BYTES, entries_crc)
    struct.pack_into("<I", raw, 16, 0)
    struct.pack_into("<I", raw, 16, zlib.crc32(raw[:92]) & 0xFFFFFFFF)
    return bytes(raw)


def _entry(type_guid: bytes, unique: bytes, first: int, last: int, name: str) -> bytes:
    raw = bytearray(GPT_ENTRY_BYTES)
    raw[:16] = type_guid
    raw[16:32] = unique
    struct.pack_into("<Q Q Q", raw, 32, first, last, 0)
    encoded = name.encode("utf-16le")
    if len(encoded) > 72:
        raise LayoutError("partition name is too long")
    raw[56:56 + len(encoded)] = encoded
    return bytes(raw)


def _canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()


def _write_at(stream: Any, lba: int, data: bytes) -> None:
    if len(data) > SECTOR and len(data) % SECTOR:
        raise LayoutError("metadata is not sector-sized")
    stream.seek(lba * SECTOR)
    stream.write(data)


def build(image: Path, plan_file: Path, *, harness_created: bool = False) -> dict[str, Any]:
    target = _image(image, harness_created)
    plan = _load_plan(plan_file)
    capacity = target.stat().st_size
    if plan["summary"]["target_bytes"] != capacity:
        raise LayoutError("plan capacity does not match image")
    last_lba = capacity // SECTOR - 1
    backup_entries = last_lba - GPT_ENTRIES_SECTORS
    usable_last = backup_entries - 1
    cursor = _align(FIRST_USABLE)
    esp_first, esp_last = cursor, cursor + ESP_BYTES // SECTOR - 1
    cursor = _align(esp_last + 1)
    partitions: list[dict[str, Any]] = [{"name": "efi", "first_lba": esp_first, "last_lba": esp_last, "bytes": ESP_BYTES, "type": "esp"}]
    for item in plan["partitions"]:
        sectors = item["size_bytes"] // SECTOR
        first, last = cursor, cursor + sectors - 1
        partitions.append({"name": item["name"], "first_lba": first, "last_lba": last, "bytes": item["size_bytes"], "type": "data"})
        cursor = _align(last + 1)
    if partitions[-1]["last_lba"] > usable_last:
        raise LayoutError("ESP and data regions exceed image capacity")
    disk_guid = _guid(_canonical({"target": plan["target_disk"], "bytes": capacity}))
    entries = bytearray(GPT_ENTRIES * GPT_ENTRY_BYTES)
    for index, item in enumerate(partitions):
        type_guid = uuid.UUID("c12a7328-f81f-11d2-ba4b-00a0c93ec93b").bytes_le if item["type"] == "esp" else _guid(b"woven-data-type")
        entries[index * GPT_ENTRY_BYTES:(index + 1) * GPT_ENTRY_BYTES] = _entry(type_guid, _guid(disk_guid + item["name"].encode()), item["first_lba"], item["last_lba"], item["name"])
    entries_crc = zlib.crc32(entries) & 0xFFFFFFFF
    metadata = {"schema": "woven-file-layout-v1", "mode": "disposable-file-test-only", "disk_guid": disk_guid.hex(), "image_bytes": capacity, "partitions": partitions, "physical_access": "none"}
    metadata_digest = hashlib.sha256(_canonical(metadata)).hexdigest()
    metadata["sha256"] = metadata_digest
    metadata_bytes = METADATA_MAGIC + len(_canonical(metadata)).to_bytes(4, "little") + _canonical(metadata)
    if len(metadata_bytes) > (esp_first - FIRST_USABLE) * SECTOR:
        raise LayoutError("metadata region is too small")
    with target.open("r+b") as stream:
        stream.seek(0)
        stream.write(b"\0" * capacity)
        mbr = bytearray(SECTOR)
        mbr[446:462] = struct.pack("<B 3s B 3s I I", 0, b"\0\x02\0", 0xEE, b"\xff\xff\xff", 1, min(last_lba, 0xFFFFFFFF))
        mbr[510:512] = b"\x55\xaa"
        _write_at(stream, 0, bytes(mbr))
        _write_at(stream, 1, _header(1, last_lba, FIRST_USABLE, usable_last, disk_guid, 2, entries_crc))
        _write_at(stream, 2, bytes(entries))
        _write_at(stream, FIRST_USABLE, metadata_bytes + b"\0" * (SECTOR - len(metadata_bytes) % SECTOR) if len(metadata_bytes) % SECTOR else metadata_bytes)
        _write_at(stream, backup_entries, bytes(entries))
        _write_at(stream, last_lba, _header(last_lba, 1, FIRST_USABLE, usable_last, disk_guid, backup_entries, entries_crc))
        stream.flush()
        os.fsync(stream.fileno())
    return verify(target, plan_file)


def verify(image: Path, plan_file: Path) -> dict[str, Any]:
    target = _image(image, True)
    plan = _load_plan(plan_file)
    capacity = target.stat().st_size
    if plan["summary"]["target_bytes"] != capacity:
        raise LayoutError("plan capacity does not match image")
    with target.open("rb") as stream:
        data = stream.read()
    if data[510:512] != b"\x55\xaa" or data[450] != 0xEE or data[512:520] != b"EFI PART":
        raise LayoutError("protective MBR or primary GPT signature is invalid")
    last_lba = capacity // SECTOR - 1
    if data[last_lba * SECTOR:last_lba * SECTOR + 8] != b"EFI PART":
        raise LayoutError("backup GPT signature is invalid")
    metadata_start = FIRST_USABLE * SECTOR
    marker = data[metadata_start:metadata_start + len(METADATA_MAGIC)]
    if marker != METADATA_MAGIC:
        raise LayoutError("layout metadata is missing")
    length = int.from_bytes(data[metadata_start + len(METADATA_MAGIC):metadata_start + len(METADATA_MAGIC) + 4], "little")
    raw = data[metadata_start + len(METADATA_MAGIC) + 4:metadata_start + len(METADATA_MAGIC) + 4 + length]
    try:
        metadata = json.loads(raw.decode())
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise LayoutError("layout metadata is invalid") from exc
    supplied = metadata.pop("sha256", None)
    if supplied != hashlib.sha256(_canonical(metadata)).hexdigest():
        raise LayoutError("layout metadata checksum mismatch")
    regions = [(item["first_lba"], item["last_lba"], item["name"]) for item in metadata["partitions"]]
    if any(first < FIRST_USABLE or last >= last_lba or first > last or first % ALIGN_SECTORS for first, last, _ in regions):
        raise LayoutError("partition bounds or alignment are invalid")
    ordered = sorted(regions)
    if any(a[1] >= b[0] for a, b in zip(ordered, ordered[1:])):
        raise LayoutError("partition regions overlap")
    return {"valid": True, "mode": metadata["mode"], "partition_count": len(regions), "metadata_sha256": supplied, "image_sha256": hashlib.sha256(data).hexdigest()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True, type=Path)
    parser.add_argument("--validated-plan", required=True, type=Path)
    parser.add_argument("--test-harness-created", action="store_true")
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    try:
        result = verify(args.image, args.validated_plan) if args.verify else build(args.image, args.validated_plan, harness_created=args.test_harness_created)
    except (LayoutError, OSError) as exc:
        print(json.dumps({"valid": False, "error": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
