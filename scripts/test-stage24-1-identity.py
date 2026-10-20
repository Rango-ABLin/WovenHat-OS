"""Run the Stage 24.1 bounded WovenIdentity core acceptance gate."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--stage", type=int, choices=(1, 2, 3, 4, 5), default=1)
    parser.add_argument(
        "--qemu",
        default=shutil.which("qemu-system-x86_64")
        or r"C:\Program Files\qemu\qemu-system-x86_64.exe",
    )
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--timeout", type=float, default=300)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu/--firmware or install QEMU with OVMF firmware.")

    env = dict(os.environ)
    env["CARGO_INCREMENTAL"] = "0"
    image = Path(
        subprocess.check_output(
            [
                "cargo",
                "run",
                "--quiet",
                "--features",
                f"stage24-{args.stage}-test",
                "--",
                "--print-image",
            ],
            cwd=root,
            env=env,
            text=True,
        ).strip()
    )
    out = root / "audit-artifacts" / f"stage24.{args.stage}-{args.cpus}cpu-{time.time_ns()}"
    out.mkdir(parents=True)
    serial = out / "serial.log"
    disk = out / "woven-identity-state.img"
    if args.stage == 3:
        with disk.open("wb") as handle:
            handle.truncate(16 * 1024 * 1024)
    command = [
        str(qemu),
        "-accel",
        "tcg,tb-size=128",
        "-machine",
        "pc",
        "-m",
        "256M",
        "-smp",
        str(args.cpus),
        "-display",
        "none",
        "-serial",
        f"file:{serial}",
        "-no-reboot",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive",
        f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
        "-drive",
        f"if=none,id=boot,format=raw,readonly=on,file={image}",
        "-device",
        "virtio-blk-pci,drive=boot,bootindex=1",
    ]
    if args.stage == 3:
        command.extend(["-drive", f"if=ide,index=0,format=raw,file={disk}"])
    try:
        with (out / "qemu.log").open("w") as log:
            result = subprocess.run(
                command,
                cwd=root,
                stdout=log,
                stderr=subprocess.STDOUT,
                timeout=args.timeout,
            ).returncode
    except subprocess.TimeoutExpired:
        print(f"Stage 24.1 timed out. Evidence: {out}", file=sys.stderr)
        return 1
    serial_text = serial.read_text(errors="replace") if serial.exists() else ""
    if args.stage == 1:
        marker = "[S24.1] WovenIdentity core: PASSED"
    elif args.stage == 2:
        marker = "[S24.2] WovenIdentity snapshot: PASSED"
    elif args.stage == 3:
        marker = "[S24.3] WovenIdentity durable storage: PASSED"
    elif args.stage == 4:
        marker = "[S24.4] WovenIdentity task binding: PASSED"
    else:
        marker = "[S24.5] WovenIdentity syscall ABI: PASSED"
    if result != 33 or marker not in serial_text:
        print(f"Stage 24.{args.stage} failed (exit {result}). Evidence: {out}", file=sys.stderr)
        print(serial_text[-8000:], file=sys.stderr)
        return 1
    print(f"Stage 24.{args.stage} PASS ({args.cpus} CPU, exit 33). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
