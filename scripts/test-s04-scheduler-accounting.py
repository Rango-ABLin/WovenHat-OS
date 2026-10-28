"""Run the bounded S04 scheduler accounting acceptance gate."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

from qemu_validation import validation_errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or
                        r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    parser.add_argument("--firmware", type=Path)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("QEMU and firmware files must exist")

    out = root / "audit-artifacts" / f"s04-scheduler-accounting-{args.cpus}cpu-{time.time_ns()}"
    out.mkdir(parents=True)
    build = subprocess.run(
        ["cargo", "run", "--quiet", "--features", "s04-scheduler-accounting-test", "--", "--print-image"],
        cwd=root, text=True, capture_output=True,
    )
    (out / "build.log").write_text(build.stdout + build.stderr, encoding="utf-8")
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1

    serial = out / "serial.log"
    command = [
        str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "4G",
        "-smp", str(args.cpus), "-display", "none", "-serial", f"file:{serial}",
        "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
    ]
    if args.cpus >= 2:
        command.extend([
            "-object", "memory-backend-ram,id=mem0,size=2G",
            "-object", "memory-backend-ram,id=mem1,size=2G",
            "-numa", "node,nodeid=0,cpus=0-" + str((args.cpus // 2) - 1) + ",memdev=mem0",
            "-numa", "node,nodeid=1,cpus=" + str(args.cpus // 2) + "-" + str(args.cpus - 1) + ",memdev=mem1",
        ])
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    try:
        with (out / "qemu.log").open("w", encoding="utf-8") as output:
            result = subprocess.run(command, cwd=root, stdout=output, stderr=output,
                                    timeout=args.timeout, creationflags=flags)
    except subprocess.TimeoutExpired:
        print(f"S04 scheduler accounting timeout; evidence: {out}", file=sys.stderr)
        return 1

    log = serial.read_text(errors="replace") if serial.exists() else ""
    errors = validation_errors(
        result.returncode,
        log,
        [f"[SMP] online={args.cpus} expected={args.cpus}",
         "[BOOT] priority round-robin fairness and per-task quanta verified",
         "[S04.SCHEDULER] accounting: PASSED",
         "[S04.TOPOLOGY] qualification: PASSED"],
    )
    if args.cpus >= 2 and f"[S04.TOPOLOGY] qualification: PASSED domains=2" not in log:
        errors.append("multi-node topology marker missing")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        print(f"FAILED; evidence: {out}", file=sys.stderr)
        return 1
    print(f"S04 scheduler accounting: PASS ({args.cpus} CPUs, exit 33). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
