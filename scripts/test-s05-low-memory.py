"""Run the bounded S05 memory/reclaim gate on the disposable low-RAM profile."""

from __future__ import annotations

import argparse
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
FIRMWARE_DEFAULT = r"C:\Program Files\qemu\share\edk2-x86_64-code.fd"
MARKER = "[S05.MEMORY] deterministic pressure/OOM policy: PASSED"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    parser.add_argument("--firmware", type=Path, default=Path(FIRMWARE_DEFAULT))
    parser.add_argument("--memory", default="256M")
    parser.add_argument("--cpus", type=int, nargs="+", choices=(1, 2, 4), default=(1, 2, 4))
    parser.add_argument("--timeout", type=float, default=240)
    args = parser.parse_args()
    for cpus in args.cpus:
        command = [
            sys.executable,
            str(ROOT / "scripts" / "test-feature-qemu.py"),
            "--feature",
            "s05-memory-pressure-test",
            "--name",
            "s05-low-memory",
            "--firmware",
            str(args.firmware),
            "--qemu",
            args.qemu,
            "--memory",
            args.memory,
            "--cpus",
            str(cpus),
            "--timeout",
            str(args.timeout),
            "--require",
            MARKER,
        ]
        result = subprocess.run(command, cwd=ROOT)
        if result.returncode != 0:
            return result.returncode
    print(f"S05 low-memory matrix: PASS ({args.memory}, CPUs={args.cpus})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
