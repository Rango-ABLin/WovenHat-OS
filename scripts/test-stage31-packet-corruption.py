"""Run the Stage 31 deterministic corrupted-packet boundary on QEMU."""

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--timeout", type=int, default=300)
    args = parser.parse_args()
    command = [
        sys.executable,
        str(ROOT / "scripts" / "test-network-qemu.py"),
        "--feature",
        "stage14-2-test",
        "--cpus",
        str(args.cpus),
        "--timeout",
        str(args.timeout),
    ]
    result = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=args.timeout + 30,
    )
    evidence = ROOT / "audit-artifacts" / f"stage31-packet-corruption-{args.cpus}cpu"
    evidence.mkdir(parents=True, exist_ok=True)
    (evidence / "command.json").write_text(
        json.dumps({"command": command, "cpus": args.cpus}, indent=2) + "\n",
        encoding="utf-8",
    )
    (evidence / "stdout-stderr.log").write_text(result.stdout, encoding="utf-8")
    source = ROOT / "target" / f"network-regression-{args.cpus}-debug"
    if source.is_dir():
        shutil.copytree(source, evidence / source.name, dirs_exist_ok=True)
    if result.returncode:
        print(result.stdout, file=sys.stderr)
        print(f"Stage 31 packet-corruption boundary failed; evidence: {evidence}", file=sys.stderr)
        return result.returncode
    serial = source / "serial.log"
    log = serial.read_text(errors="replace") if serial.exists() else ""
    marker = "[S14.2E] WovenNet ICMPv6 checksum boundary: PASSED"
    if marker not in log:
        print(f"missing checksum rejection marker; evidence: {evidence}", file=sys.stderr)
        return 1
    print(f"Stage 31 packet-corruption boundary: PASS ({args.cpus} CPU)")
    print("Coverage: deterministic malformed ICMPv6 checksum rejection")
    print(f"Evidence: {evidence}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
