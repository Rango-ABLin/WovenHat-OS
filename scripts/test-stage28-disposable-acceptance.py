#!/usr/bin/env python3
"""Run the bounded Stage 28 disposable-image acceptance pipeline.

The harness creates only regular temporary files.  It composes the existing
planner, layout builder, FAT32-compatible formatter, EFI artifact preparer,
and file-backed executor without changing those components.  The executor is
validated on a disposable transaction image because its established contract
replaces a whole file with a mock filesystem; the final partitioned image is
then assembled independently and verified end to end.

This is not physical installation, hardware qualification, ISO generation,
bootability qualification, or release certification.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
from typing import Any


ROOT = Path(__file__).parents[1]
IMAGE_BYTES = 64 * 1024 * 1024
PARTITION_BYTES = 48 * 1024 * 1024


def _load(name: str, filename: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / filename)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {filename}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


planner = _load("stage28_planner", "validate-installer-plan.py")
layout = _load("stage28_layout", "build-file-image-layout.py")
filesystem = _load("stage28_filesystem", "format-file-image-fs.py")
efi = _load("stage28_efi", "prepare-uefi-boot-artifact.py")
executor = _load("stage28_executor", "execute-file-install.py")


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()


def _make_input(image: Path) -> dict[str, Any]:
    return {
        "target_disk": {"path": f"file-image:{_digest(image)}", "serial": "STAGE28-DISPOSABLE-001"},
        "inventory": [{
            "path": f"file-image:{_digest(image)}",
            "serial": "STAGE28-DISPOSABLE-001",
            "size_bytes": IMAGE_BYTES,
            "model": "WovenHat disposable test image",
            "vendor": "WovenHat",
            "media": "file",
            "bus": "test",
            "system": False,
            "boot": False,
            "removable": False,
            "read_only": False,
            "protected": False,
        }],
        "filesystem": "fat32",
        "partitions": [{"name": "root", "size_bytes": PARTITION_BYTES}],
        "settings": {
            "user": {"username": "banda"},
            "timezone": "UTC",
            "keyboard": "us",
            "network": {"mode": "disabled"},
        },
    }


def _write_initial(path: Path) -> None:
    with path.open("wb") as stream:
        stream.truncate(IMAGE_BYTES)


def _assert_rollback(operation: Any, image: Path, *args: Any, failures: tuple[str, ...], harness_created: bool = False, extra: dict[str, Any] | None = None) -> None:
    original = image.read_bytes()
    options = dict(extra or {})
    for failure in failures:
        try:
            if harness_created:
                operation(image, *args, inject_failure=failure, harness_created=True, **options)
            else:
                operation(image, *args, inject_failure=failure, **options)
        except Exception as exc:  # component-specific fail-closed exception
            if "injected failure" not in str(exc):
                raise AssertionError(f"unexpected {failure} error: {exc}") from exc
        else:
            raise AssertionError(f"{failure} did not fail")
        if image.read_bytes() != original:
            raise AssertionError(f"{failure} did not restore the image")


def run_acceptance() -> dict[str, Any]:
    """Build and verify one deterministic disposable-image acceptance run."""
    with tempfile.TemporaryDirectory(prefix="wovenhat-stage28-") as directory:
        root = Path(directory)
        image = root / "final-disposable.img"
        _write_initial(image)

        raw_input = _make_input(image)
        input_path = root / "plan-input.json"
        input_path.write_text(json.dumps(raw_input, sort_keys=True), encoding="utf-8")
        plan_path = root / "validated-plan.json"
        plan_path.write_text(planner.render(raw_input), encoding="utf-8")
        plan_document = json.loads(plan_path.read_text(encoding="utf-8"))
        if plan_document.get("valid") is not True:
            raise AssertionError("planner did not produce a valid plan")

        executor_image = root / "executor-disposable.img"
        executor_image.write_bytes(image.read_bytes())
        executor_success = executor.execute(executor_image, plan_path)
        if not executor_success["success"] or not executor_success["verified"]:
            raise AssertionError("file-backed executor success was not verified")
        executor_manifest_digest = hashlib.sha256(_canonical(executor_success["manifest"])).hexdigest()
        executor_image_digest = _digest(executor_image)

        executor_rollback_image = root / "executor-rollback.img"
        executor_rollback_image.write_bytes(image.read_bytes())
        _assert_rollback(executor.execute, executor_rollback_image, plan_path,
                         failures=("before-replace", "after-replace"))

        layout_result = layout.build(image, plan_path, harness_created=True)
        _assert_rollback(filesystem.format_image, image, plan_path,
                         failures=("before-write", "after-write", "after-verify"), harness_created=True)
        filesystem_result = filesystem.format_image(image, plan_path, harness_created=True)
        _assert_rollback(efi.prepare, image, plan_path,
                         failures=("before-write", "after-write", "after-verify"), harness_created=True,
                         extra={"fixture": True})
        efi_result = efi.prepare(image, plan_path, fixture=True, harness_created=True)

        final_layout = layout.verify(image, plan_path)
        final_filesystem = filesystem.verify(image, plan_path)
        final_efi = efi.verify(image, plan_path)
        manifest = {
            "schema": "woven-stage28-disposable-acceptance-v1",
            "mode": "disposable-file-image-test-only",
            "physical_access": "none",
            "mounts": "none",
            "layout_sha256": final_layout["metadata_sha256"],
            "gpt_image_sha256": final_layout["image_sha256"],
            "fat32_data_sha256": final_filesystem["image_sha256"],
            "efi_artifact_sha256": final_efi["artifact_sha256"],
            "efi_path": final_efi["path"],
            "executor_manifest_sha256": executor_manifest_digest,
            "executor_image_sha256": executor_image_digest,
            "executor_rollback_verified": True,
            "formatter_rollback_verified": True,
            "efi_rollback_verified": True,
            "bootability": "not-qualified",
            "release_certification": "not-performed",
        }
        manifest_path = root / "stage28-acceptance-manifest.json"
        manifest_path.write_bytes(_canonical(manifest))
        manifest_digest = _digest(manifest_path)
        result = {
            "valid": True,
            "pipeline": "planner-layout-fat32-efi-executor",
            "image_sha256": _digest(image),
            "layout": final_layout,
            "filesystem": final_filesystem,
            "efi": final_efi,
            "executor": {
                "image_sha256": executor_image_digest,
                "manifest_sha256": executor_manifest_digest,
                "rollback_verified": True,
            },
            "manifest_sha256": manifest_digest,
            "physical_access": "none",
            "release_certification": "not-performed",
        }
        if layout_result["valid"] is not True or filesystem_result["valid"] is not True or efi_result["valid"] is not True:
            raise AssertionError("pipeline component did not report valid")
        return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit the deterministic acceptance summary")
    args = parser.parse_args()
    try:
        result = run_acceptance()
    except (AssertionError, OSError, ValueError, RuntimeError) as exc:
        print(json.dumps({"valid": False, "error": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps(result, sort_keys=True, indent=2 if args.json else None))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
