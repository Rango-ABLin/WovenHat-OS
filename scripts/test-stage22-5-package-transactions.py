"""Run the Ring-3 WovenPackage install/remove/rollback gate on 1/2/4 CPUs."""

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
    parser.add_argument(
        "--qemu",
        default=shutil.which("qemu-system-x86_64")
        or r"C:\Program Files\qemu\qemu-system-x86_64.exe",
    )
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--size-mib", type=int, default=16)
    parser.add_argument("--timeout", type=float, default=300)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
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
    build_env = dict(env)
    build_env["CARGO_TARGET_DIR"] = str(root / "target" / "wapp-packager")
    broker_image = root / "target" / "package-transaction-test" / "woven-package.wapp"
    fixture = root / "target" / "package-transaction-test" / "fixture.wpkg"
    broker_image.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(root / "tools" / "wapp-packager" / "Cargo.toml"),
            "--",
            str(elf),
            str(broker_image),
        ],
        cwd=root,
        env=build_env,
        check=True,
    )
    # Keep host packager artifacts isolated from the WAPP producer, and
    # preserve stderr plus return code when the native executable crashes.
    package_env = dict(build_env)
    package_env["CARGO_TARGET_DIR"] = str(root / "target" / "wpkg-packager")
    package_env["RUST_BACKTRACE"] = "full"
    manifest = str(root / "tools" / "wpkg-packager" / "Cargo.toml")
    subprocess.run(
        ["cargo", "build", "--manifest-path", manifest],
        cwd=root,
        env=package_env,
        check=True,
    )
    host_binary = Path(package_env["CARGO_TARGET_DIR"]) / "debug" / (
        "wovenhat-wpkg-packager.exe" if os.name == "nt" else "wovenhat-wpkg-packager"
    )
    packager_command = [
        str(host_binary),
        str(elf),
        str(fixture),
        "--name",
        "fixture",
        "--version",
        "1",
    ]
    diagnostic_dir = root / "audit-artifacts" / "stage22.5-packager"
    diagnostic_dir.mkdir(parents=True, exist_ok=True)
    with (diagnostic_dir / "packager.log").open("w") as log:
        packager = subprocess.run(
            packager_command,
            cwd=root,
            env=package_env,
            stdout=log,
            stderr=subprocess.STDOUT,
            check=False,
        )
    if packager.returncode != 0:
        print(
            f"WPKG packager exited {packager.returncode}. "
            f"Diagnostic log: {diagnostic_dir / 'packager.log'}",
            file=sys.stderr,
        )
        print(
            (diagnostic_dir / "packager.log").read_text(errors="replace")[-8000:],
            file=sys.stderr,
        )
        return 1
    if not fixture.is_file() or fixture.stat().st_size == 0:
        print("WPKG packager returned success without a fixture", file=sys.stderr)
        return 1
    build_env["WOVENHAT_PACKAGE_IMAGE"] = str(broker_image)
    build_env["WOVENHAT_PACKAGE_FIXTURE"] = str(fixture)
    image = Path(
        subprocess.check_output(
            [
                "cargo",
                "run",
                "--quiet",
                "--features",
                "stage22-5-test",
                "--",
                "--print-image",
            ],
            cwd=root,
            env=build_env,
            text=True,
        ).strip()
    )

    out = root / "audit-artifacts" / f"stage22.5-{args.cpus}cpu-{time.time_ns()}"
    out.mkdir(parents=True)
    disk = out / "woven-package-transactions.img"
    with disk.open("wb") as handle:
        handle.truncate(args.size_mib * 1024 * 1024)
    serial = out / "serial.log"
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
        "-drive",
        f"if=ide,index=0,format=raw,file={disk}",
    ]
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
        print(f"Stage 22.5 timed out. Evidence: {out}", file=sys.stderr)
        return 1
    serial_text = serial.read_text(errors="replace") if serial.exists() else ""
    marker = "[S22.5] WovenPackage transactions: PASSED"
    if result != 33 or marker not in serial_text:
        print(f"Stage 22.5 failed (exit {result}). Evidence: {out}", file=sys.stderr)
        print(serial_text[-8000:], file=sys.stderr)
        return 1
    print(f"Stage 22.5 PASS ({args.cpus} CPU, exit 33). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
