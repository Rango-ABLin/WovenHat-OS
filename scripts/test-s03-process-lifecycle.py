"""Run the bounded S03 process-lifecycle QEMU acceptance gate."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys

from qemu_validation import validation_errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--qemu",
        default=shutil.which("qemu-system-x86_64")
        or r"C:\Program Files\qemu\qemu-system-x86_64.exe",
    )
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu and --firmware to existing files.")

    image = subprocess.check_output(
        [
            "cargo",
            "run",
            "--quiet",
            "--features",
            "s03-process-lifecycle-test",
            "--",
            "--print-image",
        ],
        cwd=root,
        text=True,
    ).strip()
    out = root / "target" / f"s03-process-lifecycle-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    qemu_log = out / "qemu.log"
    serial.write_text("", encoding="utf-8")
    command = [
        str(qemu),
        "-accel",
        "tcg,tb-size=128",
        "-machine",
        "q35",
        "-m",
        "4G",
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
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    try:
        with qemu_log.open("w", encoding="utf-8") as output:
            result = subprocess.run(
                command,
                cwd=root,
                stdout=output,
                stderr=subprocess.STDOUT,
                timeout=args.timeout,
                creationflags=flags,
            )
    except subprocess.TimeoutExpired:
        print(f"S03 QEMU timed out; evidence: {out}", file=sys.stderr)
        return 1
    log = serial.read_text(errors="replace")
    errors = validation_errors(
        result.returncode,
        log,
        [
            f"[SMP] online={args.cpus} expected={args.cpus}",
            "[S03.PROCESS-LIFECYCLE] fork/wait/reap and process-group signal teardown: PASSED",
        ],
    )
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        return 1
    print(f"S03 process lifecycle: PASS ({args.cpus} CPU, exit 33)")
    print(f"Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
