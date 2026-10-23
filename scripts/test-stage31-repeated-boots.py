"""Run one Stage 31 QEMU suite repeatedly and preserve every boot's evidence.

The suites intentionally run sequentially: their QEMU harnesses use fixed
ports and shared target paths.  This wrapper never treats a missing marker or
non-zero child exit as a pass, and it snapshots the child output before
returning the failure to the caller.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SUITES = ("memory", "storage", "network", "security", "shell", "recovery", "update-service")
CPUS = (1, 2, 4, 8, 16)


def build_command(suite: str, cpus: int, timeout: int) -> list[str]:
    if suite not in SUITES:
        raise ValueError(f"unknown suite: {suite}")
    if cpus not in CPUS:
        raise ValueError(f"unsupported CPU count: {cpus}")
    if suite in ("recovery", "update-service") and cpus not in (1, 2, 4):
        raise ValueError(f"the {suite} gate currently supports only 1/2/4 CPUs")
    scripts = {
        "recovery": "scripts/test-stage27-5-recovery.py",
        "update-service": "scripts/test-stage27-5-update-service.py",
    }
    script = scripts.get(suite, f"scripts/test-{suite}-qemu.py")
    command = [sys.executable, script, "--cpus", str(cpus)]
    if suite != "shell":
        command.extend(("--timeout", str(timeout)))
    return command


def newest_shell_evidence(cpus: int) -> list[Path]:
    root = ROOT / "target" / "shell-regression"
    if not root.is_dir():
        return []
    prefix = f"diagnostic-{cpus}cpu-"
    return sorted(
        (path for path in root.iterdir() if path.name.startswith(prefix)),
        key=lambda path: path.stat().st_mtime_ns,
        reverse=True,
    )[:1]


def evidence_sources(suite: str, cpus: int) -> list[Path]:
    if suite == "shell":
        return newest_shell_evidence(cpus)
    if suite == "security":
        path = ROOT / "audit-artifacts" / f"woven-security-{cpus}cpu"
    elif suite == "recovery":
        path = ROOT / "audit-artifacts" / f"stage27.5-recovery-{cpus}cpu"
    elif suite == "update-service":
        path = ROOT / "audit-artifacts" / f"stage27.5-{cpus}cpu"
    elif suite == "network":
        path = ROOT / "target" / f"network-regression-{cpus}-debug"
    else:
        path = ROOT / "target" / f"{suite}-regression-{cpus}-debug"
    return [path] if path.exists() else []


def evidence_directory(suite: str, cpus: int, count: int) -> Path:
    suffix = "" if count == 10 else f"-{count}boots"
    return ROOT / "audit-artifacts" / "stage31-repeated" / f"{suite}-{cpus}cpu{suffix}"


def snapshot(run_dir: Path, suite: str, cpus: int, command: list[str], result: subprocess.CompletedProcess[str] | None, error: str | None) -> None:
    run_dir.mkdir(parents=True, exist_ok=True)
    (run_dir / "command.json").write_text(
        json.dumps({"command": command, "suite": suite, "cpus": cpus}, indent=2) + "\n",
        encoding="utf-8",
    )
    if result is not None:
        (run_dir / "stdout-stderr.log").write_text(result.stdout or "", encoding="utf-8")
        status = {"returncode": result.returncode, "timed_out": False, "error": error}
    else:
        status = {"returncode": None, "timed_out": True, "error": error}
    (run_dir / "result.json").write_text(json.dumps(status, indent=2) + "\n", encoding="utf-8")
    for source in evidence_sources(suite, cpus):
        destination = run_dir / source.name
        if source.is_dir():
            shutil.copytree(source, destination, dirs_exist_ok=True)
        else:
            shutil.copy2(source, destination)


def run(args: argparse.Namespace) -> int:
    evidence_root = evidence_directory(args.suite, args.cpus, args.count)
    evidence_root.mkdir(parents=True, exist_ok=True)
    for index in range(1, args.count + 1):
        command = build_command(args.suite, args.cpus, args.timeout)
        started = time.monotonic()
        print(f"[{index}/{args.count}] {' '.join(command)}", flush=True)
        try:
            result = subprocess.run(
                command,
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                timeout=args.timeout,
            )
            snapshot(
                evidence_root / f"run-{index:04d}",
                args.suite,
                args.cpus,
                command,
                result,
                None,
            )
        except subprocess.TimeoutExpired as error:
            output = (error.stdout or b"").decode(errors="replace") if isinstance(error.stdout, bytes) else (error.stdout or "")
            snapshot(
                evidence_root / f"run-{index:04d}",
                args.suite,
                args.cpus,
                command,
                subprocess.CompletedProcess(command, 124, output),
                f"wrapper timeout after {args.timeout}s",
            )
            print(f"run {index} timed out; evidence preserved", file=sys.stderr)
            return 1
        elapsed = time.monotonic() - started
        print(f"      exit={result.returncode} elapsed={elapsed:.1f}s", flush=True)
        if result.returncode != 0:
            print(f"run {index} failed; evidence preserved", file=sys.stderr)
            return 1
    print(f"Stage 31 repeated suite: PASS ({args.count} boots, {args.suite}, {args.cpus} CPUs)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", choices=SUITES, default="memory")
    parser.add_argument("--cpus", type=int, choices=CPUS, default=1)
    parser.add_argument("--count", type=int, default=10)
    parser.add_argument("--timeout", type=int, default=300)
    args = parser.parse_args()
    if not 1 <= args.count <= 10_000:
        parser.error("--count must be between 1 and 10000")
    if args.timeout < 1:
        parser.error("--timeout must be positive")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
