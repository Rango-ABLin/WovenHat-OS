"""Run the durable WovenStore catalog acceptance on a disposable ATA image."""
import argparse
import shutil
import subprocess
import sys
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--size-mib", type=int, default=16)
    parser.add_argument("--timeout", type=float, default=240)
    parser.add_argument(
        "--qemu",
        default=shutil.which("qemu-system-x86_64")
        or r"C:\Program Files\qemu\qemu-system-x86_64.exe",
    )
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu or install QEMU with its OVMF firmware.")
    image = subprocess.check_output(
        ["cargo", "run", "--quiet", "--features", "stage21-6-test", "--", "--print-image"],
        cwd=root,
        text=True,
    ).strip()
    out = root / "audit-artifacts" / f"stage21.6-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    disk = out / "woven-store.img"
    with disk.open("wb") as handle:
        handle.truncate(args.size_mib * 1024 * 1024)
    serial = out / "serial.log"
    serial.write_text("")
    command = [
        str(qemu), "-accel", "tcg,tb-size=128", "-machine", "pc", "-m", "256M",
        "-smp", str(args.cpus), "-display", "none", "-serial", f"file:{serial}",
        "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
        "-drive", f"if=ide,index=0,format=raw,file={disk}",
    ]
    try:
        with (out / "qemu.log").open("w") as output:
            result = subprocess.run(
                command, cwd=root, stdout=output, stderr=subprocess.STDOUT,
                timeout=args.timeout,
            ).returncode
    except subprocess.TimeoutExpired:
        print(f"Stage 21.6 timed out. Evidence: {out}", file=sys.stderr)
        return 1
    log = serial.read_text(errors="replace") if serial.exists() else ""
    marker = "[S21.6] WovenStore persistence: PASSED"
    if result != 33 or marker not in log:
        print(f"Stage 21.6 failed (exit {result}). Evidence: {out}", file=sys.stderr)
        print(log[-4000:], file=sys.stderr)
        return 1
    print(f"Stage 21.6 PASS ({args.cpus} CPU, exit 33). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
