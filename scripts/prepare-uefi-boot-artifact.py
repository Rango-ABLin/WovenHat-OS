#!/usr/bin/env python3
"""Prepare and verify a bounded UEFI artifact in a disposable file-image ESP.

This is a file-image test boundary.  It writes a checksummed artifact envelope
and the exact bytes of ``EFI/BOOT/BOOTX64.EFI`` into the ESP region emitted by
``build-file-image-layout.py``.  It never mounts a filesystem, opens a device,
flashes firmware, invokes a bootloader installer, or claims kernel bootability.
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
PAYLOAD_ALIGNMENT = 4096
HEADER_MAGIC = b"WOVEN-ESP-ARTIFACT-V1\0"
ARTIFACT_PATH = "EFI/BOOT/BOOTX64.EFI"
MAX_ARTIFACT_BYTES = 4 * 1024 * 1024
MAX_IMAGE_BYTES = 128 * 1024 * 1024
FIXTURE_BYTES = b"WOVENHAT-UEFI-FIXTURE-V1\n"
DEVICE_RE = re.compile(r"(?i)(?:^|[/\\])dev[/\\]|(?:^|[/\\])physicaldrive\d*$|\\\\[.?]\\physicaldrive")
PROTECTED_RE = re.compile(r"(?i)(system|boot|removable|usb|backup|external|physicaldrive|/dev/|\\\\[.?]\\)")


class ArtifactError(ValueError):
    """A fail-closed disposable UEFI artifact error."""


def _load_layout_module() -> Any:
    source = Path(__file__).with_name("build-file-image-layout.py")
    spec = importlib.util.spec_from_file_location("file_image_layout", source)
    if spec is None or spec.loader is None:
        raise ArtifactError("layout builder is unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _image(path: Path, harness_created: bool) -> Path:
    if DEVICE_RE.search(str(path)) or PROTECTED_RE.search(str(path)):
        raise ArtifactError("physical or protected image paths are forbidden")
    try:
        stat = path.lstat()
    except OSError as exc:
        raise ArtifactError("image must already exist as an explicit test file") from exc
    if path.is_symlink() or not path.is_file():
        raise ArtifactError("image must be an existing regular non-symlink file")
    if not harness_created:
        raise ArtifactError("image requires explicit test-harness creation authorization")
    if stat.st_size < 32 * 1024 * 1024 or stat.st_size > MAX_IMAGE_BYTES or stat.st_size % SECTOR:
        raise ArtifactError("image size is outside the bounded aligned test range")
    return path.absolute()


def _canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()


def _read_layout(image: Path, plan: Path) -> tuple[Any, dict[str, Any], dict[str, Any]]:
    layout = _load_layout_module()
    try:
        result = layout.verify(image, plan)
    except (layout.LayoutError, OSError) as exc:
        raise ArtifactError(str(exc)) from exc
    with image.open("rb") as stream:
        stream.seek(layout.FIRST_USABLE * SECTOR)
        marker = stream.read(len(layout.METADATA_MAGIC))
        if marker != layout.METADATA_MAGIC:
            raise ArtifactError("layout metadata is missing")
        length = int.from_bytes(stream.read(4), "little")
        raw = stream.read(length)
    try:
        metadata = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ArtifactError("layout metadata is invalid") from exc
    supplied = metadata.pop("sha256", None)
    if supplied != hashlib.sha256(layout._canonical(metadata)).hexdigest():
        raise ArtifactError("layout metadata checksum mismatch")
    regions = [item for item in metadata.get("partitions", []) if item.get("type") == "esp"]
    if len(regions) != 1 or result.get("valid") is not True:
        raise ArtifactError("layout must contain exactly one verified ESP region")
    return layout, metadata, regions[0]


def _esp_bounds(image: Path, region: dict[str, Any]) -> tuple[int, int]:
    first, last, size = region.get("first_lba"), region.get("last_lba"), region.get("bytes")
    if not isinstance(first, int) or not isinstance(last, int) or not isinstance(size, int):
        raise ArtifactError("ESP bounds are incomplete")
    if first <= 0 or last < first or size != (last - first + 1) * SECTOR:
        raise ArtifactError("ESP bounds are invalid")
    offset = first * SECTOR
    if offset % PAYLOAD_ALIGNMENT or offset + size > image.stat().st_size:
        raise ArtifactError("ESP is outside the image or not aligned")
    if size < PAYLOAD_ALIGNMENT + len(HEADER_MAGIC) + 4 + len(FIXTURE_BYTES):
        raise ArtifactError("ESP is too small for the bounded envelope")
    return offset, size


def _artifact(path: Path | None, fixture: bool) -> tuple[bytes, str]:
    if fixture == (path is not None):
        raise ArtifactError("select exactly one explicit artifact or deterministic fixture")
    if fixture:
        return FIXTURE_BYTES, "deterministic-fixture"
    assert path is not None
    if DEVICE_RE.search(str(path)):
        raise ArtifactError("physical or device artifact paths are forbidden")
    try:
        stat = path.lstat()
    except OSError as exc:
        raise ArtifactError("artifact must be an existing regular file") from exc
    if path.is_symlink() or not path.is_file():
        raise ArtifactError("artifact must be an existing regular non-symlink file")
    if stat.st_size <= 0 or stat.st_size > MAX_ARTIFACT_BYTES:
        raise ArtifactError("artifact size is outside the bounded range")
    try:
        data = path.read_bytes()
    except OSError as exc:
        raise ArtifactError("artifact is not readable") from exc
    if len(data) != stat.st_size:
        raise ArtifactError("artifact changed while it was read")
    return data, "explicit-artifact"


def _envelope(artifact: bytes, kind: str, esp_size: int) -> bytes:
    payload_offset = PAYLOAD_ALIGNMENT
    payload_end = payload_offset + len(artifact)
    metadata = {
        "schema": "woven-esp-artifact-v1",
        "mode": "disposable-file-image-test-only",
        "path": ARTIFACT_PATH,
        "artifact_kind": kind,
        "artifact_bytes": len(artifact),
        "artifact_sha256": hashlib.sha256(artifact).hexdigest(),
        "payload_offset": payload_offset,
        "payload_end": payload_end,
        "esp_bytes": esp_size,
        "physical_access": "none",
    }
    if payload_end > esp_size:
        raise ArtifactError("artifact exceeds ESP bounds")
    metadata_bytes = _canonical(metadata)
    envelope = HEADER_MAGIC + struct.pack("<I", len(metadata_bytes)) + metadata_bytes
    if len(envelope) > payload_offset:
        raise ArtifactError("artifact envelope exceeds reserved ESP header area")
    return envelope + b"\0" * (payload_offset - len(envelope)) + artifact


def verify(image: Path, plan: Path, *, harness_created: bool = True) -> dict[str, Any]:
    image = _image(image, harness_created)
    _, _, region = _read_layout(image, plan)
    offset, size = _esp_bounds(image, region)
    with image.open("rb") as stream:
        stream.seek(offset)
        esp = stream.read(size)
    if len(esp) != size or esp[:len(HEADER_MAGIC)] != HEADER_MAGIC:
        raise ArtifactError("UEFI artifact envelope is missing")
    length_start = len(HEADER_MAGIC)
    length = struct.unpack_from("<I", esp, length_start)[0]
    metadata_start = length_start + 4
    metadata_end = metadata_start + length
    if length <= 0 or metadata_end > PAYLOAD_ALIGNMENT:
        raise ArtifactError("UEFI artifact envelope bounds are invalid")
    try:
        metadata = json.loads(esp[metadata_start:metadata_end].decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ArtifactError("UEFI artifact metadata is invalid") from exc
    if metadata.get("schema") != "woven-esp-artifact-v1" or metadata.get("path") != ARTIFACT_PATH:
        raise ArtifactError("UEFI artifact path or schema is invalid")
    artifact_bytes = metadata.get("artifact_bytes")
    payload_offset = metadata.get("payload_offset")
    payload_end = metadata.get("payload_end")
    if not isinstance(artifact_bytes, int) or not isinstance(payload_offset, int) or not isinstance(payload_end, int):
        raise ArtifactError("UEFI artifact size metadata is invalid")
    if payload_offset != PAYLOAD_ALIGNMENT or payload_end != payload_offset + artifact_bytes:
        raise ArtifactError("UEFI artifact payload bounds are invalid")
    if artifact_bytes <= 0 or artifact_bytes > MAX_ARTIFACT_BYTES or payload_end > size:
        raise ArtifactError("UEFI artifact exceeds ESP bounds")
    artifact = esp[payload_offset:payload_end]
    digest = hashlib.sha256(artifact).hexdigest()
    if digest != metadata.get("artifact_sha256"):
        raise ArtifactError("UEFI artifact checksum mismatch")
    if any(esp[payload_end:]):
        raise ArtifactError("unexpected bytes follow the UEFI artifact")
    return {
        "valid": True,
        "path": ARTIFACT_PATH,
        "artifact_bytes": artifact_bytes,
        "artifact_sha256": digest,
        "esp_first_lba": region["first_lba"],
        "esp_last_lba": region["last_lba"],
        "physical_access": "none",
        "bootability": "not-qualified",
    }


def prepare(image: Path, plan: Path, *, artifact: Path | None = None, fixture: bool = False,
            harness_created: bool = False, inject_failure: str | None = None) -> dict[str, Any]:
    image = _image(image, harness_created)
    _, _, region = _read_layout(image, plan)
    offset, size = _esp_bounds(image, region)
    data, kind = _artifact(artifact, fixture)
    envelope = _envelope(data, kind, size)
    with image.open("rb") as stream:
        stream.seek(offset)
        before = stream.read(size)
    if inject_failure == "before-write":
        raise ArtifactError("injected failure before ESP write")
    try:
        with image.open("r+b") as stream:
            stream.seek(offset)
            stream.write(envelope)
            stream.write(b"\0" * (size - len(envelope)))
            stream.flush()
            os.fsync(stream.fileno())
            if inject_failure == "after-write":
                raise ArtifactError("injected failure after ESP write")
            verify(image, plan, harness_created=True)
            if inject_failure == "after-verify":
                raise ArtifactError("injected failure after verification")
    except (OSError, ArtifactError):
        with image.open("r+b") as stream:
            stream.seek(offset)
            stream.write(before)
            stream.flush()
            os.fsync(stream.fileno())
        raise
    return verify(image, plan, harness_created=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True, type=Path)
    parser.add_argument("--validated-plan", required=True, type=Path)
    source = parser.add_mutually_exclusive_group(required=False)
    source.add_argument("--artifact", type=Path)
    source.add_argument("--fixture", action="store_true")
    parser.add_argument("--test-harness-created", action="store_true")
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--inject-failure", choices=("before-write", "after-write", "after-verify"))
    args = parser.parse_args()
    try:
        if args.verify:
            result = verify(args.image, args.validated_plan, harness_created=True)
        else:
            result = prepare(args.image, args.validated_plan, artifact=args.artifact, fixture=args.fixture,
                              harness_created=args.test_harness_created, inject_failure=args.inject_failure)
    except (ArtifactError, OSError) as exc:
        print(json.dumps({"valid": False, "error": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
