"""Experimental WovenPDF-to-WovenShell desktop probe.

Uses the existing Stage 20.2 desktop app slot with WovenPDF as the test
client. This deliberately does NOT claim Stage 20.2's original hello-gui
pixel-golden test will pass: the client and its pixels are different.
Success requires WovenPDF's own window request and a captured framebuffer.
The screenshot must still be inspected; no complete-PDF rendering claim.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time


START = "[WOVENPDF] process: STARTED"
REQUEST = "[WOVENPDF] shell: OPEN_REQUEST_SENT"


def build_app(root, dirname, binary):
    app = root / "apps" / dirname
    env = os.environ.copy()
    env.pop("CARGO_BUILD_BUILD_DIR", None)
    env["CARGO_TARGET_DIR"] = str(app / "target")
    subprocess.run(["cargo", "build", "--release", "--target",
                    "x86_64-unknown-none"], cwd=app, env=env, check=True)
    result = app / "target" / "x86_64-unknown-none" / "release" / binary
    if not result.is_file():
        raise RuntimeError(f"missing app ELF: {result}")
    return result


def capture(port, screenshot, deadline):
    while time.monotonic() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=1) as sock:
                channel = sock.makefile("rwb")
                channel.readline()
                for command in (
                    {"execute": "qmp_capabilities"},
                    {"execute": "screendump", "arguments": {"filename": str(screenshot)}},
                ):
                    channel.write((json.dumps(command) + "\n").encode())
                    channel.flush()
                    while True:
                        line = channel.readline()
                        if not line:
                            raise RuntimeError("QMP connection ended")
                        reply = json.loads(line)
                        if "error" in reply:
                            raise RuntimeError(f"QMP: {reply['error']}")
                        if "return" in reply:
                            break
                return
        except (OSError, TimeoutError):
            time.sleep(0.2)
    raise RuntimeError("QMP unavailable before timeout")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--timeout", type=int, default=300)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64"))
    parser.add_argument("--firmware", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu) if args.qemu else None
    firmware = args.firmware or (
        qemu.parent / "share" / "edk2-x86_64-code.fd" if qemu else None
    )
    if not qemu or not qemu.is_file() or not firmware or not firmware.is_file():
        parser.error("QEMU and OVMF are required; pass --qemu and --firmware")

    pdf = build_app(root, "woven-pdf", "wovenhat-pdf")
    shell = build_app(root, "woven-shell", "wovenhat-woven-shell")
    env = os.environ.copy()
    env["WOVENHAT_HELLO_GUI_IMAGE"] = str(pdf)
    env["WOVENHAT_SHELL_IMAGE"] = str(shell)
    env.setdefault("CARGO_BUILD_BUILD_DIR", str(root / ".cargo-build"))
    env.setdefault("CARGO_TARGET_DIR", str(root / "target"))
    built = subprocess.run(
        ["cargo", "run", "--quiet", "--features", "stage20-2-test",
         "--", "--print-image"],
        cwd=root, env=env, capture_output=True, text=True,
    )
    if built.returncode:
        print(built.stdout, built.stderr, file=sys.stderr)
        return 1
    disk = Path(built.stdout.strip())
    if not disk.is_file():
        raise RuntimeError(f"missing QEMU boot disk: {disk}")

    out = root / "audit-artifacts" / f"wovenpdf-desktop-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    serial.write_text("")
    screenshot = out / "desktop.ppm"
    screenshot.unlink(missing_ok=True)  # Never accept a capture from an earlier run.
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    cmd = [
        str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35",
        "-m", "256M", "-smp", str(args.cpus), "-display", "none",
        "-serial", f"file:{serial}", "-no-reboot",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={disk}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    start = time.monotonic()
    deadline = start + args.timeout
    with (out / "qemu.log").open("w") as log:
        vm = subprocess.Popen(cmd, cwd=root, stdout=log, stderr=log,
                              creationflags=flags)
        try:
            while vm.poll() is None and time.monotonic() < deadline:
                events = serial.read_text(errors="replace")
                if START in events and REQUEST in events:
                    capture(port, screenshot, min(deadline, time.monotonic() + 10))
                    break
                time.sleep(0.2)
        finally:
            if vm.poll() is None:
                vm.kill()
            vm.wait()
    events = serial.read_text(errors="replace")
    if START not in events or REQUEST not in events:
        print("FAIL: WovenPDF did not prove startup plus WOPN request.", file=sys.stderr)
        print(events[-2500:], file=sys.stderr)
        return 1
    if not screenshot.exists() or screenshot.stat().st_size < 100:
        print("FAIL: no captured framebuffer.", file=sys.stderr)
        return 1
    print(f"WovenPDF desktop probe: process and WOPN request observed ({args.cpus} CPU).")
    print(f"Screenshot: {screenshot} (visual inspection required)")
    print("NOT verified: shell WOPN acceptance, PDF page pixels, navigation or full rendering.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"WovenPDF desktop probe: FAIL ({error})", file=sys.stderr)
        sys.exit(1)
