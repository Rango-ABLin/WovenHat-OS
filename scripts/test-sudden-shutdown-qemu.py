"""Inject abrupt QEMU termination and verify the same FAT32 disk recovers.

This is a disposable QEMU qualification only.  It never selects a physical
drive and it keeps the interrupted and recovery serial logs for inspection.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone

from qemu_validation import validation_errors


EARLY_MARKER = "[STORAGE MUTATION] recovery injection armed"
STORAGE_MARKER = "[STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: PASSED"
RECOVERY_MARKERS = [
    STORAGE_MARKER,
    "[BOOT] ALL VALIDATIONS PASSED",
]


def command(qemu, firmware, image, disk, serial, cpus):
    return [
        str(qemu), "-accel", "tcg,tb-size=128", "-machine", "pc", "-m", "4G",
        "-smp", str(cpus), "-display", "none", "-serial", f"file:{serial}",
        "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
        "-drive", f"if=ide,index=0,format=raw,file={disk}",
    ]


def wait_for_marker(path, marker, process, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists() and marker in path.read_text(errors="replace"):
            return True
        if process.poll() is not None:
            return False
        time.sleep(0.01)
    return False


def run_interrupted(root, qemu, firmware, image, disk, serial, cpus, timeout):
    serial.write_text("")
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    process = subprocess.Popen(
        command(qemu, firmware, image, disk, serial, cpus),
        cwd=root,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        creationflags=flags,
    )
    if not wait_for_marker(serial, EARLY_MARKER, process, timeout):
        if process.poll() is None:
            process.kill()
            process.wait(timeout=10)
        return False, "early marker was not reached"
    # The guest emits this marker immediately before entering the live
    # mutation transaction. Kill at that boundary instead of using a wall
    # clock delay, which races differently on different CPU counts.
    if process.poll() is None:
        process.kill()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=10)
    log = serial.read_text(errors="replace") if serial.exists() else ""
    if STORAGE_MARKER in log:
        return False, "guest completed storage mutation before interruption"
    return True, log[-2000:]


def run_recovery(root, qemu, firmware, image, disk, serial, cpus, timeout):
    serial.write_text("")
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    try:
        result = subprocess.run(
            command(qemu, firmware, image, disk, serial, cpus),
            cwd=root,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=timeout,
            creationflags=flags,
        )
    except subprocess.TimeoutExpired:
        return False, "recovery boot timed out"
    log = serial.read_text(errors="replace") if serial.exists() else ""
    errors = validation_errors(result.returncode, log, RECOVERY_MARKERS)
    return not errors, "\n".join(errors) if errors else log[-2000:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4, 8, 16), default=1)
    parser.add_argument("--attempts", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument(
        "--qemu",
        default=shutil.which("qemu-system-x86_64")
        or r"C:\Program Files\qemu\qemu-system-x86_64.exe",
    )
    parser.add_argument("--firmware", type=Path)
    args = parser.parse_args()
    if args.attempts < 1:
        parser.error("--attempts must be positive")
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu and --firmware to existing QEMU/OVMF files.")

    env = dict(os.environ)
    env["CARGO_INCREMENTAL"] = "0"
    image = Path(subprocess.check_output(
        ["cargo", "run", "--quiet", "--features", "qemu-test", "--", "--print-image"],
        cwd=root, env=env, text=True,
    ).strip())
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out = root / "audit-artifacts" / f"stage31-sudden-shutdown-{args.cpus}cpu-{stamp}"
    out.mkdir(parents=True, exist_ok=False)
    disk = out / "fat32.img"
    subprocess.check_call([sys.executable, str(root / "scripts" / "create-fat32.py"), str(disk)], cwd=root)

    interrupted = 0
    for attempt in range(1, args.attempts + 1):
        serial = out / f"interrupted-{attempt}.serial.log"
        ok, detail = run_interrupted(root, qemu, firmware, image, disk, serial, args.cpus, args.timeout)
        (out / f"interrupted-{attempt}.result.txt").write_text(detail)
        if ok:
            interrupted += 1
            break
    if not interrupted:
        print(f"No interrupted storage window was captured; evidence: {out}", file=sys.stderr)
        return 1

    recovery_serial = out / "recovery.serial.log"
    ok, detail = run_recovery(root, qemu, firmware, image, disk, recovery_serial, args.cpus, args.timeout)
    (out / "recovery.result.txt").write_text(detail)
    if not ok:
        print(f"Recovery boot failed; evidence: {out}\n{detail}", file=sys.stderr)
        return 1
    print(f"Sudden-shutdown QEMU recovery PASS ({args.cpus} CPU). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
