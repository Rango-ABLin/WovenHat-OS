#!/usr/bin/env python3
"""Run a seeded, fail-closed cross-subsystem Stage 31 fault matrix.

This is an orchestration boundary, not a production fault injector. It runs
the existing isolated fault/recovery gates in a deterministic pseudo-random
order, one process at a time, and preserves every command, result, and child
output. Physical disks and firmware variables are never selected. The report
therefore proves randomized scenario scheduling and cross-subsystem evidence;
it does not claim arbitrary in-guest fault injection.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import random
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
SCENARIOS = {
    "memory": ["scripts/test-memory-qemu.py"],
    "process-crash": ["scripts/test-stage31-process-crash.py"],
    "packet-corruption": ["scripts/test-stage31-packet-corruption.py"],
    "sudden-shutdown": ["scripts/test-sudden-shutdown-qemu.py"],
    "service-restart": ["scripts/test-stage27-5-update-service.py"],
}


def run(args: argparse.Namespace) -> int:
    if not 1 <= args.count <= len(SCENARIOS):
        raise SystemExit(f"--count must be between 1 and {len(SCENARIOS)}")
    rng = random.Random(args.seed)
    order = list(SCENARIOS)
    rng.shuffle(order)
    order = order[: args.count]
    evidence = ROOT / "audit-artifacts" / f"stage31-randomized-{args.seed:x}-{args.cpus}cpu"
    evidence.mkdir(parents=True, exist_ok=True)
    (evidence / "plan.json").write_text(
        json.dumps(
            {
                "schema": "wovenhat.stage31.randomized-faults/v1",
                "seed": args.seed,
                "cpus": args.cpus,
                "order": order,
                "physical_access": "none",
                "status": "pending",
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    results = []
    for index, scenario in enumerate(order, 1):
        command = [sys.executable, str(ROOT / SCENARIOS[scenario][0]), "--cpus", str(args.cpus)]
        run_dir = evidence / f"run-{index:02d}-{scenario}"
        run_dir.mkdir(parents=True, exist_ok=True)
        (run_dir / "command.json").write_text(
            json.dumps({"scenario": scenario, "command": command}, indent=2) + "\n",
            encoding="utf-8",
        )
        started = time.monotonic()
        try:
            completed = subprocess.run(
                command,
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                timeout=args.timeout,
            )
            output = completed.stdout or ""
            returncode = completed.returncode
            error = None
        except subprocess.TimeoutExpired as exc:
            output = (exc.stdout or "") if isinstance(exc.stdout, str) else ""
            returncode = 124
            error = f"timeout after {args.timeout}s"
        (run_dir / "stdout-stderr.log").write_text(output, encoding="utf-8")
        result = {
            "scenario": scenario,
            "returncode": returncode,
            "elapsed_seconds": round(time.monotonic() - started, 3),
            "error": error,
        }
        (run_dir / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        results.append(result)
        print(f"[{index}/{len(order)}] {scenario}: exit={returncode}", flush=True)
        if returncode != 0:
            (evidence / "result.json").write_text(
                json.dumps({"valid": False, "seed": args.seed, "cpus": args.cpus, "order": order, "runs": results}, indent=2)
                + "\n",
                encoding="utf-8",
            )
            print(f"randomized Stage 31 matrix failed; evidence: {evidence}", file=sys.stderr)
            return 1
    report = {"valid": True, "seed": args.seed, "cpus": args.cpus, "order": order, "runs": results}
    (evidence / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"randomized Stage 31 scenario matrix: PASS ({args.cpus} CPU, seed={args.seed:#x})")
    print(f"Evidence: {evidence}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", type=lambda value: int(value, 0), default=0x51A7E)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--count", type=int, default=len(SCENARIOS))
    parser.add_argument("--timeout", type=int, default=600)
    args = parser.parse_args()
    if args.timeout < 1:
        parser.error("--timeout must be positive")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
