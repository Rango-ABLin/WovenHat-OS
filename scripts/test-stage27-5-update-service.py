"""Run the bounded privileged update-service acceptance on 1/2/4 CPUs."""
import argparse
import os
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
    parser.add_argument("--firmware", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu or install QEMU with its OVMF firmware.")
    env = dict(os.environ)
    env["CARGO_INCREMENTAL"] = "0"
    app = root / "apps" / "woven-update"
    app_env = dict(env)
    app_env["CARGO_TARGET_DIR"] = str(app / "target")
    subprocess.run(
        ["cargo", "build", "--release", "--target", "x86_64-unknown-none"],
        cwd=app,
        env=app_env,
        check=True,
    )
    elf = app / "target" / "x86_64-unknown-none" / "release" / "wovenhat-update"
    package = root / "target" / "woven-update.wapp"
    package.parent.mkdir(parents=True, exist_ok=True)
    package_env = dict(env)
    package_env["CARGO_TARGET_DIR"] = str(root / "target" / "wapp-packager")
    subprocess.run(
        [
            "cargo", "run", "--quiet", "--manifest-path",
            str(root / "tools" / "wapp-packager" / "Cargo.toml"), "--",
            str(elf), str(package),
        ],
        cwd=root,
        env=package_env,
        check=True,
    )
    env["WOVENHAT_UPDATE_IMAGE"] = str(package)
    image = subprocess.check_output(
        ["cargo", "run", "--quiet", "--features", "stage27-5-test", "--", "--print-image"],
        cwd=root,
        env=env,
        text=True,
    ).strip()
    out = root / "audit-artifacts" / f"stage27.5-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    disk = out / "woven-update-service.img"
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
            result = subprocess.run(command, cwd=root, stdout=output,
                                    stderr=subprocess.STDOUT, timeout=args.timeout).returncode
    except subprocess.TimeoutExpired:
        print(f"Stage 27.5 timed out. Evidence: {out}", file=sys.stderr)
        return 1
    log = serial.read_text(errors="replace") if serial.exists() else ""
    marker = "[S27.5] update service: PASSED"
    if result != 33 or marker not in log:
        print(f"Stage 27.5 failed (exit {result}). Evidence: {out}", file=sys.stderr)
        print(log[-4000:], file=sys.stderr)
        return 1
    print(f"Stage 27.5 PASS ({args.cpus} CPU, exit 33). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
