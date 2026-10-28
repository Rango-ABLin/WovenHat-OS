"""Inject one disposable ATA write error and verify fail-closed containment.

The blkdebug layer is attached only to a workspace-created FAT32 image.  The
kernel's live mutation probe must report its persistence failure and stop
before publishing a false success result.  This is an emulated device-error
boundary, not physical-media or power-loss qualification.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone

FAILURE_MARKER = "[STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: FAILED at "


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--qemu",
        default=shutil.which("qemu-system-x86_64")
        or r"C:\Program Files\qemu\qemu-system-x86_64.exe",
    )
    parser.add_argument("--firmware", type=Path, required=True)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4, 8, 16), default=1)
    parser.add_argument("--timeout", type=float, default=300)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu and --firmware to existing QEMU/OVMF files.")

    image = subprocess.check_output(
        ["cargo", "run", "--quiet", "--features", "qemu-test", "--", "--print-image"],
        cwd=root,
        text=True,
    ).strip()
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    out = root / "audit-artifacts" / f"stage31-storage-write-fault-{args.cpus}cpu-{stamp}"
    out.mkdir(parents=True, exist_ok=False)
    disk = out / "fat32.img"
    config = out / "blkdebug.conf"
    serial = out / "serial.log"
    subprocess.check_call(
        [sys.executable, str(root / "scripts" / "create-fat32.py"), str(disk)],
        cwd=root,
    )
    config.write_text(
        '[inject-error]\n'
        'event = "write_aio"\n'
        'errno = "28"\n'
        # The FAT32 mutation journal lives at sectors 26/27 for this image.
        # Restricting the injection avoids firmware probing writes before the
        # kernel reaches its explicit storage-failure boundary.
        'sector = "26"\n'
        'once = "on"\n',
        encoding="utf-8",
    )
    serial.write_text("", encoding="utf-8")

    config_ref = config.relative_to(root).as_posix()
    disk_ref = disk.relative_to(root).as_posix()
    command = [
        str(qemu),
        "-accel",
        "tcg,tb-size=128",
        "-machine",
        "pc",
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
        "-drive",
        f"if=ide,index=0,format=raw,cache=none,file=blkdebug:{config_ref}:{disk_ref}",
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    process = subprocess.Popen(
        command,
        cwd=root,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        creationflags=flags,
    )
    deadline = time.monotonic() + args.timeout
    while time.monotonic() < deadline:
        log = serial.read_text(errors="replace")
        if FAILURE_MARKER in log:
            break
        if process.poll() is not None:
            break
        time.sleep(0.05)
    else:
        log = serial.read_text(errors="replace")

    marker_seen = FAILURE_MARKER in log
    if process.poll() is None:
        process.kill()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=10)
    stderr = process.stderr.read() if process.stderr is not None else ""
    errors = []
    if not marker_seen:
        errors.append(f"Missing marker: {FAILURE_MARKER}")
    if "[BOOT] ALL VALIDATIONS PASSED" in log:
        errors.append("guest reported success after injected write failure")
    if "kernel panic" in log.lower() or "panicked at" in log.lower():
        errors.append("guest serial contains panic evidence")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        print(stderr[-4000:], file=sys.stderr)
        print(f"Evidence: {out}", file=sys.stderr)
        return 1
    print(f"Stage 31 storage write-fault containment: PASS ({args.cpus} CPU)")
    print(f"Failure marker: {next(line for line in log.splitlines() if FAILURE_MARKER in line)}")
    print(f"Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
