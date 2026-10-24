#!/usr/bin/env python3
"""Build one feature-gated kernel image and validate its QEMU markers."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys

from qemu_validation import validation_errors


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--feature", required=True)
    parser.add_argument("--name", required=True, help="stable evidence directory name")
    parser.add_argument("--require", action="append", required=True)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    parser.add_argument("--firmware", type=Path, required=True)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4, 8, 16), default=2)
    parser.add_argument("--timeout", type=float, default=240)
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()

    qemu = Path(args.qemu)
    if not qemu.is_file() or not args.firmware.is_file():
        parser.error("--qemu and --firmware must identify existing files")

    build = ["cargo", "run", "--quiet"]
    if args.release:
        build.append("--release")
    build += ["--features", args.feature, "--", "--print-image"]
    image = subprocess.check_output(build, cwd=ROOT, text=True).strip()

    mode = "release" if args.release else "debug"
    output = ROOT / "target" / f"{args.name}-{args.cpus}cpu-{mode}"
    output.mkdir(parents=True, exist_ok=True)
    serial = output / "serial.log"
    qemu_log = output / "qemu.log"
    serial.write_text("", encoding="utf-8")
    command = [
        str(qemu),
        "-accel", "tcg,tb-size=128",
        "-machine", "q35",
        "-m", "256M",
        "-smp", str(args.cpus),
        "-display", "none",
        "-serial", f"file:{serial}",
        "-no-reboot",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={args.firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=args.timeout,
            creationflags=flags,
        )
    except subprocess.TimeoutExpired as error:
        qemu_log.write_text(
            (error.stdout or "") + (error.stderr or ""), encoding="utf-8"
        )
        print(f"QEMU feature gate timed out; evidence: {output}", file=sys.stderr)
        return 1

    qemu_log.write_text(completed.stdout + completed.stderr, encoding="utf-8")
    log = serial.read_text(encoding="utf-8", errors="replace")
    errors = validation_errors(completed.returncode, log, args.require)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(f"Feature gate failed; evidence: {output}", file=sys.stderr)
        return 1
    print(f"Feature gate PASS: {args.feature}, {args.cpus} CPU(s)")
    print(f"Evidence: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
