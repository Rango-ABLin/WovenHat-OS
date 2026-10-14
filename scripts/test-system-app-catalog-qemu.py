"""Validate the signed multi-application catalog boundary on 1/2/4 CPUs."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    parser.add_argument("--firmware", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu and --firmware to existing QEMU and OVMF files.")

    package = subprocess.run(
        ["python", str(root / "scripts" / "package-system-apps.py"), "--version", "0.8.0"],
        cwd=root,
        check=False,
    )
    if package.returncode:
        return package.returncode
    output = root / "target" / "system-apps" / "0.8.0"
    manifest = json.loads((output / "manifest.json").read_text(encoding="utf-8"))
    by_id = {item["id"]: item for item in manifest["apps"]}
    env = dict(os.environ)
    env["WOVENHAT_CALCULATOR_IMAGE"] = str(output / by_id["woven-calculator"]["package"])
    env["WOVENHAT_TERMINAL_IMAGE"] = str(output / by_id["woven-terminal"]["package"])
    env["WOVENHAT_FILES_IMAGE"] = str(output / by_id["woven-files"]["package"])
    env["WOVENHAT_MONITOR_IMAGE"] = str(output / by_id["woven-monitor"]["package"])
    env["WOVENHAT_SETTINGS_IMAGE"] = str(output / by_id["woven-settings"]["package"])
    env["WOVENHAT_SECURITY_IMAGE"] = str(output / by_id["woven-security"]["package"])
    env.setdefault("CARGO_BUILD_BUILD_DIR", str(root / ".cargo-build"))
    env.setdefault("CARGO_TARGET_DIR", str(root / "target"))
    boot = subprocess.run(
        ["cargo", "run", "--quiet", "--features", "stage19-app-catalog-test", "--", "--print-image"],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    if boot.returncode:
        print(boot.stdout, boot.stderr)
        return boot.returncode

    out = root / "audit-artifacts" / f"system-app-catalog-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    command = [
        str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "256M",
        "-smp", str(args.cpus), "-display", "none", "-serial", f"file:{serial}",
        "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={boot.stdout.strip()}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
    ]
    try:
        with (out / "qemu.log").open("w", encoding="utf-8") as log:
            code = subprocess.run(command, cwd=root, stdout=log, stderr=log, timeout=args.timeout).returncode
    except subprocess.TimeoutExpired:
        print(f"QEMU timeout; evidence: {out}")
        return 1
    text = serial.read_text(errors="replace") if serial.exists() else ""
    if code != 33 or "[S19-APP] six signed applications installed PASS" not in text:
        print(f"system app catalog failed (exit {code}); evidence: {out}")
        return 1
    print(f"system app catalog: PASS ({args.cpus} CPU, exit 33). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
