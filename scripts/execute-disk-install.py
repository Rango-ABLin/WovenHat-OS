#!/usr/bin/env python3
"""Execute a real, atomic, rollback-safe disposable-image install transaction.

This chains the existing GPT layout builder, FAT32-compatible formatter, and
EFI artifact preparer into one whole-image transaction driven directly from a
validated plan, instead of the three components being invoked independently.
Unlike ``execute-file-install.py`` (which replaces the entire image with an
opaque deterministic mock blob), this performs the genuine partition table,
filesystem metadata, and EFI artifact writes those components already prove
individually, and embeds the plan's normalized settings as a real file in the
formatted volume's root directory.

This remains a disposable-file-image boundary only: it never opens a device
path, never mounts anything, and does not claim bootability, hardware
qualification, or release certification. Only the ``fat32`` filesystem choice
is supported; ``ext4``/``wovenfs`` real formatting is not implemented.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import tempfile
from typing import Any


ROOT = Path(__file__).parents[1]
DEVICE_RE = re.compile(r"(?i)(?:^|[/\\])dev[/\\]|(?:^|[/\\])physicaldrive\d*$|\\\\[.?]\\physicaldrive")
PROTECTED_IDENTITY_RE = re.compile(r"(?i)(system|boot|removable|usb|backup|external|physicaldrive|/dev/|\\\\[.?]\\)")


class InstallError(ValueError):
    """A fail-closed disk-install transaction error."""


def _load(name: str, filename: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / filename)
    if spec is None or spec.loader is None:
        raise InstallError(f"unable to load {filename}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


layout = _load("disk_install_layout", "build-file-image-layout.py")
filesystem = _load("disk_install_filesystem", "format-file-image-fs.py")
efi = _load("disk_install_efi", "prepare-uefi-boot-artifact.py")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _image(path: Path) -> Path:
    if DEVICE_RE.search(str(path)):
        raise InstallError("device paths are forbidden")
    try:
        stat = path.lstat()
    except OSError as exc:
        raise InstallError(f"image cannot be inspected: {path}") from exc
    if not stat or not path.is_file() or path.is_symlink():
        raise InstallError("image must be an existing regular non-symlink file")
    return path.absolute()


def _load_plan(path: Path, image_digest: str, image_size: int) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise InstallError("validated plan must be a regular file")
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise InstallError("validated plan is not readable JSON") from exc
    if not isinstance(document, dict) or document.get("valid") is not True:
        raise InstallError("plan is not marked valid by the planner")
    plan = document.get("plan")
    if not isinstance(plan, dict) or plan.get("schema") != "woveninstaller-plan-v1":
        raise InstallError("plan schema is not woveninstaller-plan-v1")
    if plan.get("partition_table") != "gpt":
        raise InstallError("plan is not a GPT plan")
    if plan.get("filesystem") != "fat32":
        raise InstallError("only the fat32 filesystem is supported by the real executor")
    operations = plan.get("operations")
    if not isinstance(operations, dict) or operations.get("disk_io") != "none":
        raise InstallError("plan is not a file-only validated plan")
    target = plan.get("target_disk")
    if not isinstance(target, dict) or not isinstance(target.get("path"), str):
        raise InstallError("plan target identity is missing")
    identity = target["path"]
    if PROTECTED_IDENTITY_RE.search(identity) or not identity.startswith("file-image:"):
        raise InstallError("plan target identity is protected or not file-backed")
    if target.get("protected") is True or target.get("system") is True or target.get("boot") is True:
        raise InstallError("plan target is protected")
    if identity != f"file-image:{image_digest}":
        raise InstallError("plan target identity does not match image digest")
    summary = plan.get("summary")
    if not isinstance(summary, dict) or summary.get("target_bytes") != image_size:
        raise InstallError("plan target capacity does not match image size")
    if not isinstance(plan.get("partitions"), list) or not plan["partitions"]:
        raise InstallError("plan has no partitions")
    settings = plan.get("settings")
    if not isinstance(settings, dict):
        raise InstallError("plan settings are missing")
    return plan


def execute(image: Path, plan_file: Path, *, artifact: Path | None = None, fixture: bool = False,
            inject_failure: str | None = None) -> dict[str, Any]:
    """Run the real layout+format+EFI transaction atomically on a disposable image."""
    target = _image(image)
    original_digest = _sha256(target)
    original_size = target.stat().st_size
    plan = _load_plan(plan_file, original_digest, original_size)
    parent = target.parent
    backup: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(prefix=f".{target.name}.backup-", dir=parent, delete=False) as stream:
            backup = Path(stream.name)
            with target.open("rb") as source:
                stream.write(source.read())
            stream.flush()
            os.fsync(stream.fileno())

        layout.build(target, plan_file, harness_created=True)
        if inject_failure == "after-layout":
            raise InstallError("injected failure after layout build")

        filesystem.format_image(target, plan_file, harness_created=True, settings=plan["settings"])
        if inject_failure == "after-format":
            raise InstallError("injected failure after filesystem format")

        efi.prepare(target, plan_file, artifact=artifact, fixture=fixture or artifact is None, harness_created=True)
        if inject_failure == "after-efi":
            raise InstallError("injected failure after EFI artifact install")

        layout_result = layout.verify(target, plan_file)
        filesystem_result = filesystem.verify(target, plan_file, settings=plan["settings"])
        efi_result = efi.verify(target, plan_file)
        installed_settings = filesystem.read_installed_settings(target, plan_file)
        if installed_settings != plan["settings"]:
            raise InstallError("installed settings readback does not match the plan")
        if layout_result.get("valid") is not True or filesystem_result.get("valid") is not True or efi_result.get("valid") is not True:
            raise InstallError("final verification did not report valid")

        manifest = {
            "schema": "woveninstaller-execution-v1",
            "mode": "disposable-file-image-real-executor",
            "source_image_sha256": original_digest,
            "target_identity": plan["target_disk"]["path"],
            "target_serial": plan["target_disk"]["serial"],
            "filesystem": plan["filesystem"],
            "operations": {"partition": "performed", "format": "performed", "bootloader": "performed", "settings": "embedded"},
            "mock_filesystem_replacement": False,
            "partition_preserving": True,
            "physical_device_access": "none",
            "bootability": "not-qualified",
            "release_certification": "not-performed",
        }
        return {
            "success": True,
            "verified": True,
            "manifest": manifest,
            "layout": layout_result,
            "filesystem": filesystem_result,
            "efi": efi_result,
            "image_sha256": _sha256(target),
            "rollback": False,
        }
    except Exception as exc:
        if backup is not None and target.exists():
            os.replace(backup, target)
            backup = None
        raise InstallError(str(exc)) from exc
    finally:
        if backup is not None:
            try:
                backup.unlink()
            except FileNotFoundError:
                pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True, type=Path)
    parser.add_argument("--validated-plan", required=True, type=Path)
    source = parser.add_mutually_exclusive_group(required=False)
    source.add_argument("--artifact", type=Path)
    source.add_argument("--fixture", action="store_true")
    parser.add_argument("--inject-failure", choices=("after-layout", "after-format", "after-efi"))
    args = parser.parse_args()
    try:
        result = execute(args.image, args.validated_plan, artifact=args.artifact, fixture=args.fixture, inject_failure=args.inject_failure)
    except (InstallError, OSError) as exc:
        print(json.dumps({"success": False, "error": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
