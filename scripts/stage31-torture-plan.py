#!/usr/bin/env python3
"""Build a bounded, auditable Stage 31 torture-suite execution plan.

This planner does not launch QEMU, write disks, inject faults, or claim that a
missing CPU profile passed.  It makes coverage explicit so an executor can
run one job at a time and attach serial evidence to the resulting record.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


SUPPORTED_CPUS = (1, 2, 4)
REQUESTED_CPUS = (1, 2, 4, 8, 16)
SUITES = (
    ("memory", "scripts/test-memory-qemu.py"),
    ("storage", "scripts/test-storage-qemu.py"),
    ("network", "scripts/test-network-qemu.py"),
    ("security", "scripts/test-security-qemu.py"),
    ("shell", "scripts/test-shell-qemu.py"),
)


def build_plan(cpus: tuple[int, ...] = REQUESTED_CPUS) -> dict[str, Any]:
    jobs = []
    for cpu_count in cpus:
        for suite, script in SUITES:
            supported = cpu_count in SUPPORTED_CPUS
            jobs.append({
                "id": f"{suite}-{cpu_count}cpu",
                "suite": suite,
                "cpus": cpu_count,
                "status": "pending" if supported else "blocked",
                "reason": None if supported else "QEMU acceptance harness supports only 1/2/4 CPUs",
                "command": (["python", script, "--cpus", str(cpu_count)]
                            if supported else None),
                "evidence": None,
            })
    return {
        "schema": "wovenhat.stage31-torture-plan/v1",
        "fail_closed": True,
        "randomized_fault_injection": "not-enabled",
        "requested_cpu_counts": list(cpus),
        "supported_cpu_counts": list(SUPPORTED_CPUS),
        "suites": [suite for suite, _ in SUITES],
        "jobs": jobs,
    }


def validate_plan(plan: Any) -> list[str]:
    errors = []
    if not isinstance(plan, dict) or plan.get("schema") != "wovenhat.stage31-torture-plan/v1":
        return ["invalid Stage 31 plan schema"]
    jobs = plan.get("jobs")
    if not isinstance(jobs, list) or not jobs:
        return ["jobs must be a non-empty array"]
    ids = set()
    for index, job in enumerate(jobs):
        prefix = f"jobs[{index}]"
        if not isinstance(job, dict):
            errors.append(f"{prefix} must be an object")
            continue
        job_id = job.get("id")
        if not isinstance(job_id, str) or not job_id or job_id in ids:
            errors.append(f"{prefix}.id must be non-empty and unique")
        ids.add(job_id)
        if job.get("suite") not in {suite for suite, _ in SUITES}:
            errors.append(f"{prefix}.suite is unsupported")
        cpu_count = job.get("cpus")
        if cpu_count not in REQUESTED_CPUS:
            errors.append(f"{prefix}.cpus is unsupported")
        expected_blocked = cpu_count not in SUPPORTED_CPUS
        if (job.get("status") == "blocked") != expected_blocked:
            errors.append(f"{prefix}.status does not match CPU support")
        if expected_blocked and job.get("command") is not None:
            errors.append(f"{prefix}.blocked job must not have a command")
        if not expected_blocked and not isinstance(job.get("command"), list):
            errors.append(f"{prefix}.supported job needs a command")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", nargs="+", type=int, choices=REQUESTED_CPUS, default=REQUESTED_CPUS)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    plan = build_plan(tuple(args.cpus))
    errors = validate_plan(plan)
    result = {"valid": not errors, "errors": errors, "plan": plan}
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 2 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
