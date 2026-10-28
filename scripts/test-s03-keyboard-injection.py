"""Run the S03 headless QEMU USB-HID Ctrl-C evidence gate."""

import json
import os
import argparse
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time

from qemu_validation import validation_errors


def qmp(sock: socket.socket, command: str, arguments: dict | None = None) -> None:
    request = {"execute": command}
    if arguments is not None:
        request["arguments"] = arguments
    sock.sendall((json.dumps(request) + "\n").encode())
    data = b""
    while True:
        while b"\n" not in data:
            data += sock.recv(4096)
        line, data = data.split(b"\n", 1)
        reply = json.loads(line)
        if "error" in reply:
            raise RuntimeError(f"QMP {command} failed: {reply['error']}")
        if "return" in reply:
            return


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    firmware = qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        print("Set QEMU and firmware paths before running this gate.", file=sys.stderr)
        return 1
    image = subprocess.check_output(
        ["cargo", "run", "--quiet", "--features", "s03-keyboard-injection-test", "--", "--print-image"],
        cwd=root, text=True,
    ).strip()
    # Keep each CPU profile as independent evidence; a later matrix run must
    # never overwrite the serial log from an earlier profile.
    out = root / "target" / f"s03-keyboard-injection-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    qemu_log = out / "qemu.log"
    serial.write_text("", encoding="utf-8")
    port = 45683
    command = [
        str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "4G", "-smp", str(args.cpus),
        "-display", "none", "-serial", f"file:{serial}", "-no-reboot", "-qmp",
        f"tcp:127.0.0.1:{port},server=on,wait=off", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-device", "qemu-xhci,id=xhci", "-device", "usb-kbd,bus=xhci.0,id=kbd0",
        "-device", "usb-mouse,bus=xhci.0,id=mouse0",
        "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    with qemu_log.open("w", encoding="utf-8") as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=subprocess.STDOUT, creationflags=flags)
    try:
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if b"[S03.KEYBOARD] waiting" in serial.read_bytes():
                break
            if process.poll() is not None:
                raise RuntimeError(f"QEMU exited early with {process.returncode}")
            time.sleep(0.1)
        else:
            raise RuntimeError("keyboard readiness marker did not appear")
        deadline = time.monotonic() + 20
        sock = None
        while time.monotonic() < deadline:
            try:
                sock = socket.create_connection(("127.0.0.1", port), timeout=5)
                break
            except OSError:
                time.sleep(0.1)
        if sock is None:
            raise RuntimeError("QMP endpoint did not open")
        with sock:
            sock.settimeout(10)
            qmp(sock, "qmp_capabilities")
            def key(code: str, down: bool) -> dict:
                return {"type": "key", "data": {"down": down, "key": {"type": "qcode", "data": code}}}
            # The guest exits on the VINTR delivery. Do not send release
            # events after that point: QEMU may close QMP as the isa-debug
            # exit completes, which is expected rather than an input failure.
            for code, down in (("ctrl", True), ("c", True)):
                qmp(sock, "input-send-event", {"events": [key(code, down)]})
                time.sleep(0.5)
        process.wait(timeout=60)
    except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(f"keyboard gate failed: {error}", file=sys.stderr)
        process.kill()
        process.wait()
        return 1
    log = serial.read_text(errors="replace")
    errors = validation_errors(
        process.returncode,
        log,
        ["[S03.KEYBOARD] USB HID Ctrl-C delivered SIGINT to foreground group: PASSED"],
    )
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        return 1
    print("S03 headless USB-HID keyboard injection: PASS")
    print(f"Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
