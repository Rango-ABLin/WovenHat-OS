#!/usr/bin/env python3
"""Run the Stage 31 bounded low-RAM boot matrix.

This deliberately exercises the existing complete boot/recovery validation at
the minimum disposable-memory profile used by the project.  It never selects
or formats a physical disk.  A run is successful only when every child exits
through the normal QEMU test marker and every serial log passes the shared
fail-closed validator.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
PROFILES = ("256M",)


def run(args: argparse.Namespace) -> int:
    evidence = ROOT / "audit-artifacts" / f"stage31-low-ram-{args.cpus}cpu"
    evidence.mkdir(parents=True, exist_ok=True)
    plan = {
        "schema": "wovenhat.stage31.low-ram/v1",
        "cpus": args.cpus,
        "profiles": list(PROFILES),
        "physical_access": "none",
        "status": "pending",
    }
    (evidence / "plan.json").write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
    results = []
    for index, memory in enumerate(PROFILES, 1):
        command = [
            sys.executable,
            str(ROOT / "scripts" / "test-memory-qemu.py"),
            "--cpus", str(args.cpus),
            "--memory", memory,
            "--qemu", args.qemu,
            "--firmware", str(args.firmware),
            "--timeout", str(args.timeout),
        ]
        started = time.monotonic()
        try:
            completed = subprocess.run(
                command, cwd=ROOT, text=True, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT, timeout=args.timeout + 30,
            )
            output = completed.stdout or ""
            returncode = completed.returncode
            error = None
        except subprocess.TimeoutExpired as exc:
            output = exc.stdout or ""
            returncode = 124
            error = f"timeout after {args.timeout + 30}s"
        run_dir = evidence / f"run-{index:02d}-{memory.lower()}"
        run_dir.mkdir(parents=True, exist_ok=True)
        (run_dir / "command.json").write_text(json.dumps({"command": command}, indent=2) + "\n", encoding="utf-8")
        (run_dir / "stdout-stderr.log").write_text(output, encoding="utf-8")
        result = {
            "memory": memory,
            "returncode": returncode,
            "elapsed_seconds": round(time.monotonic() - started, 3),
            "error": error,
        }
        (run_dir / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        results.append(result)
        print(f"[{index}/{len(PROFILES)}] {memory}: exit={returncode}", flush=True)
        if returncode != 0:
            report = {"valid": False, **plan, "runs": results}
            (evidence / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
            print(f"Stage 31 low-RAM matrix failed; evidence: {evidence}", file=sys.stderr)
            return 1
    report = {"valid": True, **plan, "status": "passed", "runs": results}
    (evidence / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Stage 31 low-RAM matrix: PASS ({args.cpus} CPU)")
    print(f"Evidence: {evidence}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4, 8, 16), default=1)
    parser.add_argument("--qemu", required=True)
    parser.add_argument("--firmware", required=True)
    parser.add_argument("--timeout", type=int, default=180)
    args = parser.parse_args()
    if args.timeout < 1:
        parser.error("--timeout must be positive")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
