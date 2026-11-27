"""Exercise the bounded WovenFiles mutation CLI through a real signed WAPP.

This is a software/QEMU qualification only. It reuses the existing userspace
`FileWrite` authority and mounted-path policy; no syscall, capability or ABI
change is involved.
"""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--memory", default="5G", help="guest RAM (default: 5G; lower only when host virtual memory is constrained)")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    app = root / "apps" / "woven-files"
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu and --firmware to existing QEMU and OVMF files.")

    env = dict(os.environ)
    env.pop("CARGO_BUILD_BUILD_DIR", None)
    env["CARGO_TARGET_DIR"] = str(app / "target")
    if subprocess.run(["cargo", "build", "--release"], cwd=app, env=env).returncode:
        return 1
    image = app / "target" / "x86_64-unknown-none" / "release" / "wovenhat-files"
    package = image.with_suffix(".wapp")
    package_env = dict(os.environ)
    package_env.pop("CARGO_BUILD_BUILD_DIR", None)
    package_env["CARGO_TARGET_DIR"] = str(root / "target" / "wapp-packager")
    if subprocess.run([
        "cargo", "run", "--quiet", "--manifest-path",
        str(root / "tools" / "wapp-packager" / "Cargo.toml"), "--",
        str(image), str(package),
    ], cwd=root, env=package_env).returncode:
        return 1

    env = dict(os.environ)
    env["WOVENHAT_APP_IMAGE"] = str(package)
    env.setdefault("CARGO_BUILD_BUILD_DIR", str(root / ".cargo-build"))
    env.setdefault("CARGO_TARGET_DIR", str(root / "target"))
    boot = subprocess.run([
        "cargo", "run", "--quiet", "--features", "native-app-mutation-test",
        "--", "--print-image",
    ], cwd=root, env=env, capture_output=True, text=True)
    if boot.returncode:
        print(boot.stdout, boot.stderr)
        return boot.returncode

    out = root / "audit-artifacts" / f"native-app-mutations-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    # Mutations must be qualified against mounted, persistent storage. Use a
    # disposable workspace-local FAT32 image; never touch a real device.
    disk = out / "fat32.img"
    if disk.exists():
        disk.unlink()
    subprocess.check_call(
        [sys.executable, str(root / "scripts" / "create-fat32.py"), str(disk)], cwd=root
    )
    command = [
        str(qemu), "-accel", "tcg,tb-size=128", "-machine", "pc", "-m", args.memory,
        "-smp", str(args.cpus), "-display", "none", "-serial", f"file:{serial}",
        "-no-reboot",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={boot.stdout.strip()}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
        "-drive", f"if=ide,index=0,format=raw,file={disk}",
    ]
    try:
        with (out / "qemu.log").open("w") as log:
            code = subprocess.run(
                command, cwd=root, stdout=log, stderr=log, timeout=args.timeout
            ).returncode
    except subprocess.TimeoutExpired:
        print(f"QEMU timeout; evidence: {out}")
        return 1

    text = serial.read_text(errors="replace") if serial.exists() else ""
    expected = [
        "[STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: PASSED",
        "[NATIVE-APP] --mkdir PASS",
        "[NATIVE-APP] --write PASS",
        "[NATIVE-APP] --rename PASS",
        "[NATIVE-APP] --rm PASS",
        "[NATIVE-APP] persistent mount mutations: PASSED",
        "[NATIVE-APP] bounded file mutations: PASSED",
    ]
    missing = [marker for marker in expected if marker not in text]
    if code != 33 or missing:
        print(f"WovenFiles mutations failed (exit {code}, missing {missing}); evidence: {out}")
        return 1
    print(f"WovenFiles mutations: PASS ({args.cpus} CPU, exit 33). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
