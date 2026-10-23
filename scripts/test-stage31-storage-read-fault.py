"""Inject one disposable ATA read error and verify fail-closed boot recovery.

The blkdebug layer is attached only to a workspace-created FAT32 image.  This
test never selects a physical disk and accepts the expected RAM-VFS fallback,
and accepts either a successful retry or a fail-closed RAM-VFS fallback.  It
does not treat either outcome as physical-media qualification.
"""

from __future__ import annotations

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
    out = root / "audit-artifacts" / f"stage31-storage-read-fault-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    disk = out / "fat32.img"
    config = out / "blkdebug.conf"
    serial = out / "serial.log"
    subprocess.check_call(
        [sys.executable, str(root / "scripts" / "create-fat32.py"), str(disk)],
        cwd=root,
    )
    config.write_text(
        '[inject-error]\n'
        'event = "read_aio"\n'
        'errno = "5"\n'
        'once = "on"\n',
        encoding="utf-8",
    )
    serial.write_text("", encoding="utf-8")

    # Keep both paths relative to the workspace because blkdebug's legacy
    # filename form uses colon-separated config/image components.
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
        f"if=ide,index=0,format=raw,file=blkdebug:{config_ref}:{disk_ref}",
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    try:
        result = subprocess.run(
            command,
            cwd=root,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            timeout=args.timeout,
            creationflags=flags,
        )
    except subprocess.TimeoutExpired:
        print(f"storage read-fault boot timed out; evidence: {out}", file=sys.stderr)
        return 1

    log = serial.read_text(errors="replace")
    required = [
        f"[SMP] online={args.cpus} expected={args.cpus}",
        "[BOOT] ALL VALIDATIONS PASSED",
    ]
    errors = validation_errors(result.returncode, log, required)
    storage_markers = (
        "[STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: PASSED",
        "[STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: SKIPPED",
    )
    if not any(marker in log for marker in storage_markers):
        errors.append("missing storage recovery outcome marker")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        print(result.stderr[-4000:], file=sys.stderr)
        print(f"Evidence: {out}", file=sys.stderr)
        return 1
    outcome = "retry" if storage_markers[0] in log else "fallback"
    print(f"Stage 31 storage read-fault recovery: PASS ({args.cpus} CPU, {outcome})")
    print(f"Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
