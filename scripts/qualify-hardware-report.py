#!/usr/bin/env python3
"""Validate and classify a WovenHat native-hardware qualification report.

The tool is deliberately report-only: it never probes devices, changes host
state, flashes media, or treats QEMU evidence as native hardware evidence.
Each observation must identify its category, execution environment, result,
and an evidence path.  The resulting tier is conservative and suitable for a
checked-in compatibility-lab record.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any


CATEGORIES = (
    "intel-pc", "amd-pc", "laptop", "desktop", "nvme", "sata", "usb",
    "ethernet", "wifi", "audio", "intel-graphics", "amd-graphics",
    "nvidia-graphics", "touchpad", "webcam", "bluetooth", "multi-monitor",
)
RESULTS = {"passed", "failed", "skipped", "blocked"}
ENVIRONMENTS = {"native", "qemu", "emulated"}
TIERS = {"tier1", "tier2", "tier3"}


def _text(value: Any) -> str:
    return value.strip() if isinstance(value, str) else ""


def _repository_relative_evidence_path(evidence: str) -> bool:
    """Apply the same fail-closed path policy on Linux and Windows runners.

    Native Path.is_absolute() on Linux does not recognize Windows drive or
    UNC paths. Evidence uses repository-relative POSIX separators only.
    This is syntactic validation; consumers must still prevent symlink escapes.
    """
    if not evidence or "\\" in evidence or "\x00" in evidence:
        return False
    posix = PurePosixPath(evidence)
    windows = PureWindowsPath(evidence)
    return (
        not posix.is_absolute()
        and not windows.anchor
        and ".." not in posix.parts
        and posix.parts != ()
    )


def validate(report: Any) -> list[str]:
    """Return all schema/policy errors without executing any test."""
    errors: list[str] = []
    if not isinstance(report, dict):
        return ["report must be a JSON object"]
    if report.get("schema") != "wovenhat.hardware-qualification/v1":
        errors.append("schema must be wovenhat.hardware-qualification/v1")
    host = report.get("host")
    if not isinstance(host, dict):
        errors.append("host must be an object")
    else:
        if not _text(host.get("host_id")):
            errors.append("host.host_id is required")
        if host.get("environment") not in ENVIRONMENTS:
            errors.append("host.environment must be native, qemu, or emulated")
    observations = report.get("observations")
    if not isinstance(observations, list) or not observations:
        errors.append("observations must be a non-empty array")
        return errors
    seen: set[str] = set()
    for index, item in enumerate(observations):
        prefix = f"observations[{index}]"
        if not isinstance(item, dict):
            errors.append(f"{prefix} must be an object")
            continue
        observation_id = _text(item.get("id"))
        if not observation_id:
            errors.append(f"{prefix}.id is required")
        elif observation_id in seen:
            errors.append(f"{prefix}.id is duplicated")
        seen.add(observation_id)
        category = _text(item.get("category"))
        if category not in CATEGORIES:
            errors.append(f"{prefix}.category is unsupported")
        if item.get("result") not in RESULTS:
            errors.append(f"{prefix}.result is invalid")
        if item.get("environment") not in ENVIRONMENTS:
            errors.append(f"{prefix}.environment is invalid")
        if not _text(item.get("device")):
            errors.append(f"{prefix}.device is required")
        evidence = _text(item.get("evidence"))
        if not evidence:
            errors.append(f"{prefix}.evidence is required")
        elif not _repository_relative_evidence_path(evidence):
            errors.append(f"{prefix}.evidence must be a repository-relative path")
        if item.get("tier") == "tier1" and not (
            item.get("environment") == "native" and item.get("result") == "passed"
        ):
            errors.append(f"{prefix}.tier1 requires passed native evidence")
        if item.get("tier") is not None and item.get("tier") not in TIERS:
            errors.append(f"{prefix}.tier is invalid")
    return errors


def classify(report: dict[str, Any]) -> dict[str, Any]:
    """Return a conservative tier summary.  Call validate first."""
    observations = report["observations"]
    native_passed = {item["category"] for item in observations
                     if item["environment"] == "native" and item["result"] == "passed"}
    failed = [item["id"] for item in observations if item["result"] == "failed"]
    blocked = [item["id"] for item in observations if item["result"] == "blocked"]
    missing = sorted(set(CATEGORIES) - native_passed)
    if failed:
        tier = "tier3"
        reason = "native qualification has failed observations"
    elif native_passed == set(CATEGORIES):
        tier = "tier1"
        reason = "all declared qualification categories passed natively"
    else:
        tier = "tier2"
        reason = "no native failure is recorded, but native coverage is incomplete"
    return {
        "schema": "wovenhat.hardware-qualification-result/v1",
        "host_id": report["host"]["host_id"],
        "tier": tier,
        "reason": reason,
        "native_passed_categories": sorted(native_passed),
        "missing_native_categories": missing,
        "failed_observations": failed,
        "blocked_observations": blocked,
        "observation_count": len(observations),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        report = json.loads(args.input.read_text(encoding="utf-8-sig"))
    except (OSError, json.JSONDecodeError) as exc:
        parser.error(str(exc))
    errors = validate(report)
    if errors:
        result: dict[str, Any] = {"valid": False, "errors": errors}
        code = 2
    else:
        result = {"valid": True, "classification": classify(report)}
        code = 0
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return code


if __name__ == "__main__":
    raise SystemExit(main())
