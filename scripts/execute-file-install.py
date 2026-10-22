#!/usr/bin/env python3
"""Execute a bounded, disposable file-backed installer plan.

This is a test fixture only. It replaces one explicitly supplied regular file
with a deterministic mock filesystem image. It never accepts a device path
and never mounts, formats, partitions, or installs a bootloader.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import tempfile
from typing import Any


MAX_IMAGE_BYTES = 64 * 1024 * 1024
MAX_PAYLOAD_BYTES = 4 * 1024 * 1024
MAGIC = b"WOVEN-MOCK-FS-V1\n"
DEVICE_RE = re.compile(r"(?i)(?:^|[/\\])dev[/\\]|(?:^|[/\\])physicaldrive\d*$|\\\\[.?]\\physicaldrive")
PROTECTED_IDENTITY_RE = re.compile(r"(?i)(system|boot|removable|usb|backup|external|physicaldrive|/dev/|\\\\[.?]\\)")


class ExecutionError(ValueError):
    """A fail-closed executor validation or transaction error."""


def _regular_image(path: Path) -> Path:
    if DEVICE_RE.search(str(path)):
        raise ExecutionError("device paths are forbidden")
    try:
        stat = path.lstat()
    except OSError as exc:
        raise ExecutionError(f"image cannot be inspected: {path}") from exc
    if not stat or not path.is_file() or path.is_symlink():
        raise ExecutionError("image must be an existing regular non-symlink file")
    if stat.st_size <= 0 or stat.st_size > MAX_IMAGE_BYTES:
        raise ExecutionError("image size is outside the bounded test range")
    return path.absolute()


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_plan(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise ExecutionError("validated plan must be a regular file")
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ExecutionError("validated plan is not readable JSON") from exc
    if not isinstance(document, dict) or document.get("valid") is not True:
        raise ExecutionError("plan is not marked valid by the planner")
    plan = document.get("plan")
    if not isinstance(plan, dict) or plan.get("schema") != "woveninstaller-plan-v1":
        raise ExecutionError("plan schema is not woveninstaller-plan-v1")
    operations = plan.get("operations")
    if not isinstance(operations, dict) or operations.get("disk_io") != "none":
        raise ExecutionError("plan is not a file-only validated plan")
    target = plan.get("target_disk")
    if not isinstance(target, dict) or not isinstance(target.get("path"), str):
        raise ExecutionError("plan target identity is missing")
    identity = target["path"]
    if PROTECTED_IDENTITY_RE.search(identity) or not identity.startswith("file-image:"):
        raise ExecutionError("plan target identity is protected or not file-backed")
    if target.get("protected") is True or target.get("system") is True or target.get("boot") is True:
        raise ExecutionError("plan target is protected")
    if not isinstance(target.get("serial"), str) or not target["serial"]:
        raise ExecutionError("plan target serial is missing")
    if not isinstance(plan.get("partitions"), list) or not plan["partitions"]:
        raise ExecutionError("plan has no partitions")
    if plan.get("filesystem") not in ("ext4", "fat32", "wovenfs"):
        raise ExecutionError("plan filesystem is unsupported")
    return plan


def _canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode("utf-8")


def _build_payload(plan: dict[str, Any], source_digest: str, image_size: int) -> tuple[bytes, dict[str, Any]]:
    target = plan["target_disk"]
    if target["path"] != f"file-image:{source_digest}":
        raise ExecutionError("plan target identity does not match image digest")
    if plan.get("summary", {}).get("target_bytes") != image_size:
        raise ExecutionError("plan target capacity does not match image size")
    manifest = {
        "schema": "woveninstaller-execution-v1",
        "mode": "disposable-file-test-only",
        "source_image_sha256": source_digest,
        "target_identity": target["path"],
        "target_serial": target["serial"],
        "filesystem": plan["filesystem"],
        "partitions": plan["partitions"],
        "settings": plan["settings"],
        "physical_device_access": "none",
        "mount_format_partition_bootloader": "not-implemented",
    }
    manifest_bytes = _canonical(manifest)
    mock_tree = {
        "etc/os-release": "NAME=WovenHat Mock\nINSTALLER_MODE=disposable-file-test-only\n",
        "etc/woven-installer.json": manifest,
        "home/user/README.txt": "This is a deterministic mock filesystem payload.\n",
    }
    payload = MAGIC + len(manifest_bytes).to_bytes(4, "little") + manifest_bytes + _canonical(mock_tree)
    if len(payload) > MAX_PAYLOAD_BYTES or len(payload) > image_size:
        raise ExecutionError("deterministic mock payload exceeds bounded image")
    return payload, manifest


def _verify(path: Path, expected_manifest: dict[str, Any], payload: bytes) -> str:
    with path.open("rb") as stream:
        actual = stream.read(len(payload))
    if actual != payload:
        raise ExecutionError("installed mock payload verification failed")
    if _sha256(path) == "":
        raise ExecutionError("installed image digest is empty")
    embedded = payload[len(MAGIC) + 4:]
    if expected_manifest["source_image_sha256"] not in embedded.decode("utf-8"):
        raise ExecutionError("installed manifest verification failed")
    return _sha256(path)


def execute(image: Path, plan_file: Path, inject_failure: str | None = None) -> dict[str, Any]:
    """Atomically install the mock payload, restoring the original on failure."""
    target = _regular_image(image)
    plan = _load_plan(plan_file)
    original_digest = _sha256(target)
    payload, manifest = _build_payload(plan, original_digest, target.stat().st_size)
    parent = target.parent
    backup: Path | None = None
    backup_ready = False
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(prefix=f".{target.name}.backup-", dir=parent, delete=False) as stream:
            backup = Path(stream.name)
            with target.open("rb") as source:
                shutil.copyfileobj(source, stream)
            stream.flush()
            os.fsync(stream.fileno())
            backup_ready = True
        with tempfile.NamedTemporaryFile(prefix=f".{target.name}.staging-", dir=parent, delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(payload)
            stream.write(b"\0" * (target.stat().st_size - len(payload)))
            stream.flush()
            os.fsync(stream.fileno())
        if inject_failure == "before-replace":
            raise ExecutionError("injected failure before atomic replacement")
        os.replace(temporary, target)
        temporary = None
        if inject_failure == "after-replace":
            raise ExecutionError("injected failure after atomic replacement")
        digest = _verify(target, manifest, payload)
        return {"success": True, "verified": True, "manifest": manifest, "image_sha256": digest, "rollback": False}
    except Exception as exc:
        if backup_ready and backup is not None and target.exists():
            os.replace(backup, target)
            backup = None
        raise ExecutionError(str(exc)) from exc
    finally:
        for candidate in (temporary, backup):
            if candidate is not None:
                try:
                    candidate.unlink()
                except FileNotFoundError:
                    pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True, type=Path)
    parser.add_argument("--validated-plan", required=True, type=Path)
    parser.add_argument("--inject-failure", choices=("before-replace", "after-replace"))
    args = parser.parse_args()
    try:
        result = execute(args.image, args.validated_plan, args.inject_failure)
    except (ExecutionError, OSError) as exc:
        print(json.dumps({"success": False, "error": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
