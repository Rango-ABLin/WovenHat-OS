"""Verify the Stage 32.13 generation-bound GDB owner."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys

from qemu_validation import validation_errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4, 8, 16), default=1)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu and --firmware to existing files.")
    image = subprocess.check_output(
        ["cargo", "run", "--quiet", "--features", "stage32-13-test", "--", "--print-image"],
        cwd=root, text=True,
    ).strip()
    out = root / "target" / f"stage32-13-gdb-owner-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    qemu_log = out / "qemu.log"
    serial.write_text("")
    command = [
        str(qemu), "-accel", "tcg", "-machine", "q35", "-m", "256M",
        "-smp", str(args.cpus), "-display", "none", "-serial", f"file:{serial}",
        "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    try:
        result = subprocess.run(command, cwd=root, capture_output=True, text=True,
                                timeout=args.timeout, creationflags=flags)
    except subprocess.TimeoutExpired:
        print(f"stage32-13 QEMU timed out; evidence: {out}", file=sys.stderr)
        return 1
    qemu_log.write_text(result.stdout + result.stderr, encoding="utf-8")
    log = serial.read_text(errors="replace")
    errors = validation_errors(result.returncode, log, [
        f"[SMP] online={args.cpus} expected={args.cpus}",
        "[S32.13] GDB owner: PASSED",
    ])
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        return 1
    print(f"Stage 32.13 GDB owner: PASS ({args.cpus} CPU, exit 33)")
    print(f"Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
