"""Experimental WovenPDF-to-WovenShell desktop probe.

Uses the existing Stage 20.2 desktop app slot with WovenPDF as the test
client. This deliberately does NOT claim Stage 20.2's original hello-gui
pixel-golden test will pass: the client and its pixels are different.
Success requires the shell's window acceptance and WovenPDF's distinctive
page, chrome and accent colors in the framebuffer. This is not proof of
PDF document text rendering; the Stage 20.2 client has no document path.
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
ACCEPTED = "woven-shell: window opened: WovenPDF Preview"


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



def framebuffer_palette(path):
    """Count near-white pixels and WovenPDF's exact GUI palette in QEMU P6."""
    with path.open("rb") as stream:
        def token():
            while True:
                char = stream.read(1)
                if not char:
                    raise ValueError("truncated PPM header")
                if char == b"#":
                    stream.readline()
                    continue
                if not char.isspace():
                    break
            result = bytearray(char)
            while True:
                char = stream.read(1)
                if not char or char.isspace():
                    return bytes(result)
                result.extend(char)
                if len(result) > 32:
                    raise ValueError("oversized PPM token")

        if token() != b"P6":
            raise ValueError("expected binary P6 screenshot")
        width, height, maximum = int(token()), int(token()), int(token())
        if not (320 <= width <= 4096 and 200 <= height <= 2160 and maximum == 255):
            raise ValueError("invalid screenshot dimensions or channel range")
        pixels = stream.read()
        if len(pixels) != width * height * 3:
            raise ValueError("truncated or oversized screenshot payload")
        near_white = page = chrome = accent = 0
        for r, g, b in zip(pixels[::3], pixels[1::3], pixels[2::3]):
            near_white += r >= 235 and g >= 235 and b >= 235
            page += (r, g, b) == (248, 249, 253)
            chrome += (r, g, b) == (22, 29, 44)
            accent += (r, g, b) == (88, 210, 176)
        return near_white, page, chrome, accent


def wovenpdf_window_visible(counts):
    # A bright desktop alone must not pass. Require the actual GUI palette.
    near_white, page, chrome, accent = counts
    return near_white >= 5000 and page >= 5000 and chrome >= 2000 and accent >= 30


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
                if START in events and REQUEST in events and ACCEPTED in events:
                    # Require the shell to accept WOPN before checking pixels.
                    # The compositor may need several
                    # frames before the window becomes visible. Capture anew
                    # until page pixels appear or the bounded grace expires.
                    grace = min(deadline, time.monotonic() + 45)
                    while vm.poll() is None and time.monotonic() < grace:
                        capture(port, screenshot, min(grace, time.monotonic() + 3))
                        try:
                            if wovenpdf_window_visible(framebuffer_palette(screenshot)):
                                break
                        except ValueError:
                            pass
                        time.sleep(0.5)
                    break
                time.sleep(0.2)
        finally:
            if vm.poll() is None:
                vm.kill()
            vm.wait()
    events = serial.read_text(errors="replace")
    if START not in events or REQUEST not in events or ACCEPTED not in events:
        print("FAIL: WovenPDF startup, WOPN request, or WovenShell acceptance missing.", file=sys.stderr)
        print(events[-2500:], file=sys.stderr)
        return 1
    if not screenshot.exists() or screenshot.stat().st_size < 100:
        print("FAIL: no captured framebuffer.", file=sys.stderr)
        return 1
    try:
        palette = framebuffer_palette(screenshot)
    except ValueError as error:
        print(f"FAIL: invalid framebuffer: {error}", file=sys.stderr)
        return 1
    if not wovenpdf_window_visible(palette):
        print(f"FAIL: WovenPDF page/chrome/accent palette missing: {palette}", file=sys.stderr)
        print(events[-2500:], file=sys.stderr)
        return 1
    print(f"Framebuffer near-white/page/chrome/accent pixels: {palette}")
    print(f"WovenPDF desktop probe: process, WOPN, and shell acceptance observed ({args.cpus} CPU).")
    print(f"Screenshot: {screenshot} (visual inspection required)")
    print("NOT verified: PDF document text pixels, navigation or full rendering.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"WovenPDF desktop probe: FAIL ({error})", file=sys.stderr)
        sys.exit(1)
