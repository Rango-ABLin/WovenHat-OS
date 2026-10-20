"""Run the bounded Stage 23.1/23.2 WovenAI acceptance gate."""

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
    parser.add_argument("--stage", type=int, choices=(1, 2, 3, 4, 5, 6), default=1)
    parser.add_argument(
        "--qemu",
        default=shutil.which("qemu-system-x86_64")
        or r"C:\Program Files\qemu\qemu-system-x86_64.exe",
    )
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--timeout", type=float, default=300)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu/--firmware or install QEMU with OVMF firmware.")

    env = dict(os.environ)
    env["CARGO_INCREMENTAL"] = "0"
    app = root / "apps" / "woven-ai"
    app_env = dict(env)
    app_env["CARGO_TARGET_DIR"] = str(app / "target")
    subprocess.run(
        ["cargo", "build", "--release", "--target", "x86_64-unknown-none"],
        cwd=app,
        env=app_env,
        check=True,
    )
    elf = app / "target" / "x86_64-unknown-none" / "release" / "wovenhat-ai"
    output = root / "target" / "ai-broker-test"
    output.mkdir(parents=True, exist_ok=True)
    packager_env = dict(env)
    packager_env["CARGO_TARGET_DIR"] = str(root / "target" / "wapp-packager")
    service_image = output / "woven-ai.wapp"
    probe_image = output / "woven-ai-probe.wapp"
    packager = root / "tools" / "wapp-packager" / "Cargo.toml"
    for destination in (service_image, probe_image):
        subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(packager),
                "--",
                str(elf),
                str(destination),
            ],
            cwd=root,
            env=packager_env,
            check=True,
        )

    build_env = dict(env)
    build_env["WOVENHAT_AI_IMAGE"] = str(service_image)
    build_env["WOVENHAT_AI_PROBE_IMAGE"] = str(probe_image)
    image = Path(
        subprocess.check_output(
            [
                "cargo",
                "run",
                "--quiet",
                "--features",
                f"stage23-{args.stage}-test",
                "--",
                "--print-image",
            ],
            cwd=root,
            env=build_env,
            text=True,
        ).strip()
    )

    out = root / "audit-artifacts" / f"stage23.{args.stage}-{args.cpus}cpu-{time.time_ns()}"
    out.mkdir(parents=True)
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
        print(f"Stage 23.{args.stage} timed out. Evidence: {out}", file=sys.stderr)
        return 1
    serial_text = serial.read_text(errors="replace") if serial.exists() else ""
    if args.stage == 1:
        marker = "[S23.1] WovenAI broker: PASSED"
    elif args.stage == 2:
        marker = "[S23.2] WovenAI local provider: PASSED"
    elif args.stage == 3:
        marker = "[S23.3] WovenAI cloud provider: PASSED"
    elif args.stage == 4:
        marker = "[S23.4] WovenAI capability security: PASSED"
    elif args.stage == 5:
        marker = "[S23.5] WovenAI semantic search: PASSED"
    else:
        marker = "[S23.6] WovenAssistant actions: PASSED"
    if result != 33 or marker not in serial_text:
        print(f"Stage 23.{args.stage} failed (exit {result}). Evidence: {out}", file=sys.stderr)
        print(serial_text[-8000:], file=sys.stderr)
        return 1
    print(f"Stage 23.{args.stage} PASS ({args.cpus} CPU, exit 33). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
