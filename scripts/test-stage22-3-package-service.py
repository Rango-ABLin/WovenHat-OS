"""Run the signed Ring-3 WovenPackage broker acceptance on 1/2/4 CPUs."""

import argparse
import os
from pathlib import Path
import subprocess
import sys
import time
import shutil


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--qemu", default=os.environ.get("WOVENHAT_QEMU", "qemu-system-x86_64"))
    parser.add_argument("--firmware")
    parser.add_argument("--timeout", type=float, default=240)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(shutil.which(args.qemu) or args.qemu)
    firmware = Path(args.firmware) if args.firmware else qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu/--firmware or install QEMU with OVMF firmware.")

    env = dict(os.environ)
    env["CARGO_INCREMENTAL"] = "0"
    app = root / "apps" / "woven-package"
    app_env = dict(env)
    app_env["CARGO_TARGET_DIR"] = str(app / "target")
    subprocess.run(
        ["cargo", "build", "--release", "--target", "x86_64-unknown-none"],
        cwd=app,
        env=app_env,
        check=True,
    )
    elf = app / "target" / "x86_64-unknown-none" / "release" / "wovenhat-package"
    package = root / "target" / "woven-package.wapp"
    package.parent.mkdir(parents=True, exist_ok=True)
    package_env = dict(env)
    package_env["CARGO_TARGET_DIR"] = str(root / "target" / "wapp-packager")
    subprocess.run(
        ["cargo", "run", "--quiet", "--manifest-path", str(root / "tools" / "wapp-packager" / "Cargo.toml"), "--", str(elf), str(package)],
        cwd=root,
        env=package_env,
        check=True,
    )
    build_env = dict(env)
    build_env["WOVENHAT_PACKAGE_IMAGE"] = str(package)
    image = Path(subprocess.check_output(
        ["cargo", "run", "--quiet", "--features", "stage22-3-test", "--", "--print-image"],
        cwd=root,
        env=build_env,
        text=True,
    ).strip())

    out = root / "audit-artifacts" / f"stage22.3-{args.cpus}cpu-{time.time_ns()}"
    out.mkdir(parents=True)
    serial = out / "serial.log"
    command = [
        str(qemu), "-accel", "tcg,tb-size=128", "-machine", "pc", "-m", "256M",
        "-smp", str(args.cpus), "-display", "none", "-serial", f"file:{serial}",
        "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
    ]
    with (out / "qemu.log").open("w") as log:
        result = subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT, timeout=args.timeout).returncode
    serial_text = serial.read_text(errors="replace") if serial.exists() else ""
    marker = "[S22.3] WovenPackage broker: PASSED"
    if result != 33 or marker not in serial_text:
        print(f"Stage 22.3 failed (exit {result}). Evidence: {out}", file=sys.stderr)
        print(serial_text[-5000:], file=sys.stderr)
        return 1
    print(f"Stage 22.3 PASS ({args.cpus} CPU, exit 33). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
