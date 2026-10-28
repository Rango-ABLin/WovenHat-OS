"""Build and run the WovenCalculator Ring-3 application through the signed ABI."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess


def app_env(app):
    env = dict(os.environ)
    env.pop("CARGO_BUILD_BUILD_DIR", None)
    env["CARGO_TARGET_DIR"] = str(app / "target")
    return env


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    parser.add_argument("--firmware", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    app = root / "apps" / "woven-calculator"
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu and --firmware to existing QEMU and OVMF files.")

    build = subprocess.run(["cargo", "build", "--release", "--target", "x86_64-unknown-none"], cwd=app, env=app_env(app))
    if build.returncode:
        return build.returncode
    image = app / "target" / "x86_64-unknown-none" / "release" / "wovenhat-calculator"
    package = image.with_suffix(".wapp")
    package_env = dict(os.environ)
    package_env.pop("CARGO_BUILD_BUILD_DIR", None)
    package_env["CARGO_TARGET_DIR"] = str(root / "target" / "wapp-packager")
    packaged = subprocess.run([
        "cargo", "run", "--quiet", "--manifest-path", str(root / "tools" / "wapp-packager" / "Cargo.toml"), "--", str(image), str(package)
    ], cwd=root, env=package_env)
    if packaged.returncode:
        return packaged.returncode

    env = dict(os.environ)
    env["WOVENHAT_APP_IMAGE"] = str(package)
    env.setdefault("CARGO_BUILD_BUILD_DIR", str(root / ".cargo-build"))
    env.setdefault("CARGO_TARGET_DIR", str(root / "target"))
    boot = subprocess.run(["cargo", "run", "--quiet", "--features", "stage16-1-test", "--", "--print-image"], cwd=root, env=env, capture_output=True, text=True)
    if boot.returncode:
        print(boot.stdout, boot.stderr)
        return boot.returncode

    out = root / "audit-artifacts" / f"woven-calculator-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    qemu_log = out / "qemu.log"
    command = [str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "256M", "-smp", str(args.cpus), "-display", "none", "-serial", f"file:{serial}", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}", "-drive", f"if=none,id=boot,format=raw,readonly=on,file={boot.stdout.strip()}", "-device", "virtio-blk-pci,drive=boot,bootindex=1"]
    try:
        with qemu_log.open("w") as log:
            code = subprocess.run(command, cwd=root, stdout=log, stderr=log, timeout=args.timeout).returncode
    except subprocess.TimeoutExpired:
        print(f"QEMU timeout; evidence: {out}")
        return 1
    text = serial.read_text(errors="replace") if serial.exists() else ""
    if code != 33 or "[S16.1] out-of-kernel application: PASSED" not in text:
        print(f"WovenCalculator failed (exit {code}); evidence: {out}")
        return 1
    print(f"WovenCalculator: PASS ({args.cpus} CPU, exit 33). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
