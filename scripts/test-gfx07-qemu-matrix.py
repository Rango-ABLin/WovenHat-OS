#!/usr/bin/env python3
"""GFX-07 QEMU regression orchestrator (not a production certification).

Runs existing graphics gates on 1/2/4 CPUs and records exact exit status,
captured output, source commit and SHA256 log hashes. Any missing prerequisite,
nonzero exit, timeout or fatal marker fails closed. Physical GPU, real HID,
GPU-backed 3D presentation, measured performance and GFX-08 are NOT covered.
"""
import argparse
import hashlib
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GATES = (
    ("desktop", "scripts/test-woven-shell-qemu.py", ("--stage", "20.4")),
    ("dual-monitor", "scripts/test-stage20-14b-desktop.py", ()),
    ("session", "scripts/test-stage20-15-session.py", ()),
)
FATAL = re.compile(r"(?im)^.*(?:kernel panic|panicked at|security violation|\bFAILED\b|\bTIMEOUT\b).*$")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", type=Path, required=True)
    parser.add_argument("--firmware", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.timeout < 1:
        parser.error("--timeout must be positive")
    for label, path in (("QEMU", args.qemu), ("OVMF", args.firmware)):
        if not path.is_file():
            parser.error(f"{label} not a file: {path}")
    # Verify the source before creating any audit artifacts inside the checkout.
    # Otherwise the runner can make its own source tree appear dirty.
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True)
    dirty = subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True)
    if head.returncode or dirty.returncode or dirty.stdout.strip():
        parser.error("cannot qualify an unknown or dirty source tree")
    output = args.output.resolve()
    if output.exists():
        parser.error("output directory already exists; refusing to overwrite evidence")
    output.mkdir(parents=True)
    manifest = {
        "schema": 1,
        "classification": "QEMU graphics regression only; NOT GFX-08 production qualification",
        "commit": head.stdout.strip(),
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "qemu": str(args.qemu.resolve()),
        "firmware": str(args.firmware.resolve()),
        "results": [],
    }
    failed = False
    try:
        for cpu in (1, 2, 4):
            for name, script, extra in GATES:
                cmd = [sys.executable, str(ROOT / script), "--cpus", str(cpu),
                       "--qemu", str(args.qemu.resolve()),
                       "--firmware", str(args.firmware.resolve()), *extra]
                timed_out = False
                try:
                    run = subprocess.run(cmd, cwd=ROOT, capture_output=True, timeout=args.timeout)
                    raw = run.stdout + b"\n" + run.stderr
                    code = run.returncode
                except subprocess.TimeoutExpired as exc:
                    timed_out = True
                    raw = (exc.stdout or b"") + b"\n" + (exc.stderr or b"")
                    code = None
                log_name = f"{name}-{cpu}cpu.log"
                (output / log_name).write_bytes(raw)
                fatal = bool(FATAL.search(raw.decode("utf-8", "replace")))
                passed = not timed_out and code == 0 and not fatal
                failed |= not passed
                manifest["results"].append({
                    "gate": name, "cpus": cpu, "command": cmd,
                    "exit_code": code, "timeout": timed_out,
                    "fatal_output_marker": fatal, "pass": passed,
                    "log": log_name, "sha256": sha256(raw),
                })
                print(f"{name} {cpu} CPU: {'PASS' if passed else 'FAIL'}", flush=True)
    finally:
        manifest["passed"] = not failed and len(manifest["results"]) == len(GATES) * 3
        (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return 1 if not manifest["passed"] else 0


if __name__ == "__main__":
    sys.exit(main())
