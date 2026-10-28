#!/usr/bin/env python3
"""Run the bounded virtual-hardware qualification matrix.

The runner expands the repository's declarative matrix in deterministic order
and invokes the pure planning/result functions from
``hardware-qualification-stimulator.py``.  It never opens a device, starts a
hypervisor, or makes a native-hardware claim.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
STIMULATOR_PATH = ROOT / "scripts" / "hardware-qualification-stimulator.py"
MATRIX_PATH = ROOT / "tests" / "hardware-matrix.json"
PHYSICAL_STATUS = "PHYSICAL_NOT_TESTED"
FORBIDDEN_OPTIONS = {
    "--device",
    "--physical",
    "--physical-device",
    "--raw",
    "--raw-disk",
    "--raw-device",
    "--native",
    "--native-hardware",
    "--tier1",
    "--tier1-hardware",
    "--physical-qualification",
}
FORBIDDEN_MARKERS = (
    "/dev/",
    "\\\\.\\physicaldrive",
    "\\\\.\\scsi",
    "\\\\.\\usb",
)


class MatrixError(ValueError):
    """Raised when a matrix request is invalid or unsafe."""


def _load_stimulator() -> Any:
    spec = importlib.util.spec_from_file_location("wovenhat_hardware_stimulator", STIMULATOR_PATH)
    if spec is None or spec.loader is None:
        raise MatrixError(f"cannot load stimulator: {STIMULATOR_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


stimulator = _load_stimulator()


def _reject_physical_arguments(argv: Iterable[str]) -> None:
    """Reject unsafe options before argparse or output-directory creation."""
    for raw in argv:
        token = raw.lower()
        option = token.split("=", 1)[0]
        if option in FORBIDDEN_OPTIONS:
            raise MatrixError(f"physical/native option is forbidden: {raw}")
        if any(marker in token for marker in FORBIDDEN_MARKERS):
            raise MatrixError(f"physical-device path is forbidden: {raw}")


def _seed(value: str) -> int:
    try:
        number = int(value, 0)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("seed must be an integer") from exc
    if not 0 <= number <= 0xFFFFFFFFFFFFFFFF:
        raise argparse.ArgumentTypeError("seed must fit in an unsigned 64-bit integer")
    return number


def _positive_int(value: str) -> int:
    try:
        number = int(value, 10)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("value must be a positive integer") from exc
    if number <= 0:
        raise argparse.ArgumentTypeError("value must be a positive integer")
    return number


def _profile_ids(matrix: dict[str, Any], requested: list[str] | None) -> list[str]:
    profiles = matrix.get("profiles")
    if not isinstance(profiles, list):
        raise MatrixError("hardware matrix has no profiles list")
    available = []
    for item in profiles:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str):
            raise MatrixError("hardware matrix contains a malformed profile")
        available.append(item["id"])
    if not requested or requested == ["all"]:
        return available
    if "all" in requested:
        raise MatrixError("--profile all cannot be combined with a profile id")
    unknown = [profile for profile in requested if profile not in available]
    if unknown:
        raise MatrixError(f"invalid profile: {', '.join(unknown)}")
    selected = set(requested)
    return [profile for profile in available if profile in selected]


def _cpu_counts(matrix: dict[str, Any], requested: list[int] | None) -> list[int]:
    allowed = set(stimulator.ALLOWED_CPUS)
    values = list(matrix.get("cpu_counts", [])) if requested is None else requested
    if not values:
        raise MatrixError("hardware matrix has no CPU counts")
    unknown = [value for value in values if value not in allowed]
    if unknown:
        raise MatrixError(f"invalid CPU count: {unknown[0]}")
    matrix_values = set(matrix.get("cpu_counts", [])) | set(matrix.get("future_cpu_counts", []))
    outside_matrix = [value for value in values if value not in matrix_values]
    if outside_matrix:
        raise MatrixError(f"CPU count is not in the hardware matrix: {outside_matrix[0]}")
    return sorted(set(values))


def _ram_sizes(matrix: dict[str, Any], requested: list[int] | None) -> list[int]:
    available = matrix.get("ram_mib", [])
    if not isinstance(available, list) or not available:
        raise MatrixError("hardware matrix has no RAM sizes")
    values = available if requested is None else requested
    unknown = [value for value in values if value not in set(available)]
    if unknown:
        raise MatrixError(f"invalid RAM size: {unknown[0]}")
    return sorted(set(values))


def _safe_output_dir(raw: str) -> Path:
    try:
        return stimulator._safe_output_dir(raw)
    except (ValueError, TypeError) as exc:
        raise MatrixError(str(exc)) from exc


def _job_name(index: int, profile: str, cpus: int, ram_mib: int) -> str:
    safe_profile = "".join(char if char.isalnum() or char in "_.-" else "_" for char in profile)
    return f"{index:04d}-{safe_profile}-{cpus}cpu-{ram_mib}mib"


def build_plan(
    matrix: dict[str, Any],
    seed: int,
    cpus: list[int] | None,
    ram_mib: list[int] | None,
    profiles: list[str] | None,
    output_dir: Path,
) -> dict[str, Any]:
    """Build the stable execution plan without touching the filesystem."""
    selected_profiles = _profile_ids(matrix, profiles)
    selected_cpus = _cpu_counts(matrix, cpus)
    selected_ram = _ram_sizes(matrix, ram_mib)
    jobs = []
    index = 0
    for profile in selected_profiles:
        for cpu_count in selected_cpus:
            for ram_size in selected_ram:
                child = Path("children") / _job_name(index, profile, cpu_count, ram_size)
                jobs.append({
                    "index": index,
                    "profile": profile,
                    "cpu_count": cpu_count,
                    "ram_mib": ram_size,
                    "output_dir": str(child),
                })
                index += 1
    return {
        "schema": 1,
        "kind": "wovenhat-hardware-qualification-matrix-plan",
        "matrix": str(MATRIX_PATH),
        "seed": seed,
        "job_count": len(jobs),
        "jobs": jobs,
        "physical_status": PHYSICAL_STATUS,
        "safety": {
            "physical_device_access": False,
            "raw_disk_access": False,
            "native_hardware_claim": False,
            "tier1_claim": False,
        },
        "output_dir": str(output_dir),
    }


def _write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _safe_result(result: dict[str, Any], job: dict[str, Any], matrix: dict[str, Any]) -> tuple[bool, str]:
    allowed = set(matrix.get("status_values", []))
    if len(result.get("profiles", [])) != 1:
        return False, "child result does not contain exactly one profile"
    record = result["profiles"][0]
    if record.get("profile") != job["profile"]:
        return False, "child profile does not match the plan"
    if record.get("cpu_count") != job["cpu_count"] or record.get("ram_mib") != job["ram_mib"]:
        return False, "child resource tuple does not match the plan"
    if record.get("status") not in allowed or record.get("status") == "BLOCKED":
        return False, "child result has an invalid or blocked status"
    if record.get("physical_status") != PHYSICAL_STATUS:
        return False, "child result did not mark physical qualification as not tested"
    safety = result.get("safety", {})
    for field in ("physical_device_access", "raw_disk_access", "native_hardware_claim", "tier1_claim"):
        if safety.get(field) is not False:
            return False, f"child result violates safety boundary: {field}"
    for field in ("physical_claim", "native_claim", "tier1_claim"):
        if record.get(field) is not False:
            return False, f"child profile violates safety boundary: {field}"
    return True, "ok"


def execute(plan: dict[str, Any], matrix: dict[str, Any], output_dir: Path, plan_only: bool) -> int:
    output_dir.mkdir(parents=True, exist_ok=True)
    _write_json(output_dir / "plan.json", plan)
    if plan_only:
        _write_json(output_dir / "summary.json", {
            "schema": 1,
            "kind": "wovenhat-hardware-qualification-matrix-summary",
            "status": "NOT_RUN",
            "job_count": plan["job_count"],
            "completed": 0,
            "failed": 0,
            "physical_status": PHYSICAL_STATUS,
            "children": [],
        })
        return 0

    children = []
    failed = 0
    for job in plan["jobs"]:
        child_dir = output_dir / job["output_dir"]
        try:
            result = stimulator.build_result(
                matrix,
                plan["seed"],
                job["cpu_count"],
                job["ram_mib"],
                [job["profile"]],
                [],
                child_dir,
            )
            valid, reason = _safe_result(result, job, matrix)
            if valid:
                for record in result["profiles"]:
                    evidence = Path(record["evidence_file"])
                    _write_json(evidence, record)
                _write_json(child_dir / "result.json", result)
            else:
                failed += 1
        except (OSError, ValueError, KeyError, TypeError) as exc:
            result = None
            valid, reason = False, f"child execution error: {exc}"
            failed += 1
        children.append({
            "index": job["index"],
            "profile": job["profile"],
            "cpu_count": job["cpu_count"],
            "ram_mib": job["ram_mib"],
            "valid": valid,
            "reason": reason,
            "result_file": str(Path(job["output_dir"]) / "result.json") if result is not None and valid else None,
        })

    _write_json(output_dir / "summary.json", {
        "schema": 1,
        "kind": "wovenhat-hardware-qualification-matrix-summary",
        "status": "PASS" if failed == 0 else "FAIL",
        "job_count": plan["job_count"],
        "completed": len(children),
        "failed": failed,
        "physical_status": PHYSICAL_STATUS,
        "children": children,
    })
    return 0 if failed == 0 else 1


def run(argv: list[str]) -> int:
    _reject_physical_arguments(argv)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan-only", action="store_true")
    parser.add_argument("--seed", type=_seed, default=0x51A7E)
    parser.add_argument("--cpus", type=int, action="append")
    parser.add_argument("--ram-mib", type=_positive_int, action="append")
    parser.add_argument("--profile", action="append")
    parser.add_argument("--output-dir", default="audit-artifacts/hardware-qualification-matrix")
    args = parser.parse_args(argv)
    matrix = stimulator.load_matrix(MATRIX_PATH)
    output_dir = _safe_output_dir(args.output_dir)
    plan = build_plan(matrix, args.seed, args.cpus, args.ram_mib, args.profile, output_dir)
    return execute(plan, matrix, output_dir, args.plan_only)


def main() -> int:
    try:
        return run(sys.argv[1:])
    except (MatrixError, OSError) as exc:
        print(f"run-hardware-qualification-matrix: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
