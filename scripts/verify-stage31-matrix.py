#!/usr/bin/env python3
"""Audit retained bounded Stage 31 cross-subsystem evidence.

This verifier never launches QEMU and never mutates historical evidence.  It
is deliberately stricter than the scenario runner: every requested CPU
profile must have a valid report, the complete required scenario set must be
present exactly once, every child must have returned zero, and each child
must retain its command, result, and output record.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
REQUIRED_SCENARIOS = frozenset(
    {
        "packet-corruption",
        "sudden-shutdown",
        "network-loss",
        "process-crash",
        "low-ram",
        "service-restart",
        "driver-restart",
        "service-supervisor",
        "memory",
    }
)


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def audit_report(path: Path, expected_cpu: int) -> list[str]:
    errors: list[str] = []
    result_path = path / "result.json"
    if not result_path.is_file():
        return [f"{path}: missing result.json"]
    try:
        report = json.loads(result_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"{result_path}: invalid JSON: {error}"]
    if report.get("valid") is not True:
        fail(errors, f"{path}: report is not valid")
    if report.get("cpus") != expected_cpu:
        fail(errors, f"{path}: expected cpus={expected_cpu}, got {report.get('cpus')!r}")
    runs = report.get("runs")
    if not isinstance(runs, list):
        return errors + [f"{path}: runs is not an array"]
    names = [run.get("scenario") for run in runs if isinstance(run, dict)]
    if len(names) != len(REQUIRED_SCENARIOS) or set(names) != REQUIRED_SCENARIOS:
        fail(errors, f"{path}: scenario set mismatch: {sorted(names)!r}")
    if len(names) != len(set(names)):
        fail(errors, f"{path}: duplicate scenarios")
    for run in runs:
        if not isinstance(run, dict):
            fail(errors, f"{path}: non-object child result")
            continue
        scenario = run.get("scenario")
        if run.get("returncode") != 0 or run.get("error") is not None:
            fail(errors, f"{path}: {scenario}: child did not pass")
        child = path / f"run-{names.index(scenario) + 1:02d}-{scenario}"
        for name in ("command.json", "result.json", "stdout-stderr.log"):
            if not (child / name).is_file():
                fail(errors, f"{path}: {scenario}: missing {name}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", nargs="+", type=int, choices=(1, 2, 4, 8, 16), default=(1, 2, 4))
    parser.add_argument("--root", type=Path, default=ROOT / "audit-artifacts")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    errors: list[str] = []
    audited: list[dict[str, object]] = []
    for cpu in args.cpus:
        candidates = sorted(args.root.glob(f"stage31-randomized-51a7e-{cpu}cpu-9scenarios"))
        if len(candidates) != 1:
            fail(errors, f"cpu={cpu}: expected exactly one canonical evidence directory, found {len(candidates)}")
            continue
        directory = candidates[0]
        report_errors = audit_report(directory, cpu)
        errors.extend(report_errors)
        audited.append({"cpus": cpu, "path": str(directory), "valid": not report_errors})
    result = {
        "schema": "wovenhat.stage31.matrix-audit/v1",
        "valid": not errors,
        "cpus": list(args.cpus),
        "required_scenarios": sorted(REQUIRED_SCENARIOS),
        "audited": audited,
        "errors": errors,
    }
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
