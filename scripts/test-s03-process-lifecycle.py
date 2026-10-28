"""Run the bounded S03 process-lifecycle QEMU acceptance gate."""

import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time

from qemu_validation import validation_errors


class Qmp:
    def __init__(self, port: int, deadline: float) -> None:
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=2)
        self.sock.settimeout(2)
        self.file = self.sock.makefile("rwb")
        self.file.readline()
        self.command("qmp_capabilities", deadline)

    def command(self, execute: str, deadline: float, arguments: dict | None = None) -> dict:
        while True:
            try:
                self.file.write((json.dumps({"execute": execute, "arguments": arguments or {}}) + "\r\n").encode())
                self.file.flush()
                while True:
                    reply = json.loads(self.file.readline())
                    if "return" in reply:
                        return reply["return"]
                    if "error" in reply:
                        raise RuntimeError(reply["error"])
                
            except (OSError, TimeoutError):
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.1)

    def ctrl_c(self, deadline: float) -> None:
        self.command(
            "input-send-event",
            deadline,
            {"events": [
                {"type": "key", "data": {"down": True, "key": {"type": "qcode", "data": "ctrl"}}},
                {"type": "key", "data": {"down": True, "key": {"type": "qcode", "data": "c"}}},
                {"type": "key", "data": {"down": False, "key": {"type": "qcode", "data": "c"}}},
                {"type": "key", "data": {"down": False, "key": {"type": "qcode", "data": "ctrl"}}},
            ]},
        )

    def close(self) -> None:
        self.file.close()
        self.sock.close()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--qemu",
        default=shutil.which("qemu-system-x86_64")
        or r"C:\Program Files\qemu\qemu-system-x86_64.exe",
    )
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("Set --qemu and --firmware to existing files.")

    image = subprocess.check_output(
        [
            "cargo",
            "run",
            "--quiet",
            "--features",
            "s03-process-lifecycle-test",
            "--",
            "--print-image",
        ],
        cwd=root,
        text=True,
    ).strip()
    out = root / "target" / f"s03-process-lifecycle-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    qemu_log = out / "qemu.log"
    qmp_port = 45500 + args.cpus
    serial.write_text("", encoding="utf-8")
    command = [
        str(qemu),
        "-accel",
        "tcg,tb-size=128",
        "-machine",
        "q35",
        "-m",
        "4G",
        "-smp",
        str(args.cpus),
        "-display",
        "none",
        "-serial",
        f"file:{serial}",
        "-qmp",
        f"tcp:127.0.0.1:{qmp_port},server=on,wait=off",
        "-no-reboot",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive",
        f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
        "-drive",
        f"if=none,id=boot,format=raw,readonly=on,file={image}",
        "-device",
        "virtio-blk-pci,drive=boot,bootindex=1",
        "-device",
        "qemu-xhci,id=xhci",
        "-device",
        "usb-kbd,bus=xhci.0,id=kbd0",
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    qmp = None
    injected = False
    process = None
    try:
        with qemu_log.open("w", encoding="utf-8") as output:
            process = subprocess.Popen(
                command,
                cwd=root,
                stdout=output,
                stderr=subprocess.STDOUT,
                creationflags=flags,
            )
            started = time.monotonic()
            while process.poll() is None:
                if time.monotonic() - started > args.timeout:
                    print(f"S03 QEMU timed out; evidence: {out}", file=sys.stderr)
                    process.kill()
                    process.wait()
                    return 1
                if qmp is None:
                    try:
                        qmp = Qmp(qmp_port, time.monotonic() + 2)
                    except (OSError, TimeoutError, RuntimeError):
                        time.sleep(0.1)
                        continue
                log = serial.read_text(errors="replace") if serial.exists() else ""
                marker = "[S03.VINTR] waiting for injected Ctrl-C foreground interrupt"
                if not injected and marker in log:
                    qmp.ctrl_c(time.monotonic() + 5)
                    injected = True
                time.sleep(0.1)
            result_returncode = process.returncode
    finally:
        if qmp is not None:
            qmp.close()
    log = serial.read_text(errors="replace")
    if "[S03.VINTR] waiting for injected Ctrl-C foreground interrupt" not in log:
        print("S03 QEMU did not publish the VINTR probe marker", file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        return 1
    errors = validation_errors(
        result_returncode,
        log,
        [
            f"[SMP] online={args.cpus} expected={args.cpus}",
            "[S03.PROCESS-LIFECYCLE] fork/wait/reap and process-group signal teardown: PASSED",
        ],
    )
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        return 1
    print(f"S03 process lifecycle: PASS ({args.cpus} CPU, exit 33)")
    print(f"Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
