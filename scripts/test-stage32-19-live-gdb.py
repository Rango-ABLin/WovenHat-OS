"""Qualify a real GDB RSP query/continue exchange over QEMU COM2."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import socket
import shutil
import subprocess
import sys
import time

from qemu_validation import validation_errors


def packet(payload: bytes) -> bytes:
    return b"$" + payload + b"#%02x" % (sum(payload) & 0xFF)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    parser.add_argument("--firmware", type=Path, required=True)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--timeout", type=float, default=45)
    parser.add_argument("--feature", default="stage32-19-test")
    parser.add_argument("--name", default=None)
    parser.add_argument("--command", default="c", choices=("c", "D", "k"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    image = subprocess.check_output(
        ["cargo", "run", "--quiet", "--features", args.feature, "--", "--print-image"],
        cwd=root, text=True,
    ).strip()
    out = root / "target" / f"{args.name or 'stage32-19-live-gdb'}-{args.cpus}cpu"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    qemu_log = out / "qemu.log"
    serial.write_text("", encoding="utf-8")
    # Reserve a local ephemeral TCP port for QEMU's second legacy UART.
    probe = socket.socket()
    probe.bind(("127.0.0.1", 0))
    port = probe.getsockname()[1]
    probe.close()
    command = [
        str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "256M",
        "-smp", str(args.cpus), "-display", "none", "-serial", f"file:{serial}",
        "-serial", f"tcp:127.0.0.1:{port},server=on,wait=off", "-no-reboot",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={args.firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1",
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    process = subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               creationflags=flags)
    client = None
    try:
        deadline = time.monotonic() + args.timeout
        while client is None and time.monotonic() < deadline:
            try:
                candidate = socket.create_connection(("127.0.0.1", port), timeout=1)
                candidate.settimeout(1)
                client = candidate
            except OSError:
                time.sleep(0.05)
        if client is None:
            raise RuntimeError("QEMU COM2 TCP endpoint did not open")
        # Do not transmit before the guest has initialized COM2 and reached
        # the scheduler-owned qualification hook; early UART bytes are not
        # buffered by the kernel's bounded polling path.
        ready_marker = (
            f"[S32.{20 if args.command == 'D' else 21}] live GDB {'detach' if args.command == 'D' else 'kill'}: READY"
            if args.command in ("D", "k")
            else "[S10.3] userspace async completion ABI + cancellation/teardown: PASSED"
        )
        while time.monotonic() < deadline:
            if ready_marker in serial.read_text(encoding="utf-8", errors="replace"):
                break
            time.sleep(0.02)
        # The INT3 self-test polls COM2. Ask for the stop reason, verify the
        # reply, then continue so the kernel can publish its acceptance marker.
        received = bytearray()
        next_query = 0.0
        while b"S05" not in received and time.monotonic() < deadline:
            if time.monotonic() >= next_query:
                client.sendall(packet(b"?"))
                # Keep at most one outstanding query in the bounded guest
                # poll loop; flooding it can starve the subsequent D packet.
                next_query = time.monotonic() + 1.0
            try:
                received.extend(client.recv(256))
            except socket.timeout:
                continue
        if b"S05" not in received:
            raise RuntimeError(f"missing GDB stop reply: {received!r}")
        # The bounded kernel entry consumes complete `$...#cc` frames directly;
        # it does not feed standalone host ACK bytes into that frame buffer.
        # Sending `+` here would prefix the next packet and make it invalid.
        if args.command != "D":
            client.sendall(b"+")
        client.sendall(packet(args.command.encode()))
        if args.command == "D":
            while b"OK" not in received and time.monotonic() < deadline:
                try:
                    received.extend(client.recv(256))
                except socket.timeout:
                    continue
            if b"OK" not in received:
                raise RuntimeError(f"missing detach acknowledgement: {received!r}")
        remaining = max(0.1, deadline - time.monotonic())
        stdout, stderr = process.communicate(timeout=remaining)
    except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
        if process.poll() is None:
            process.kill()
        stdout, stderr = process.communicate()
        qemu_log.write_text((stdout or b"").decode(errors="replace") + (stderr or b"").decode(errors="replace"), encoding="utf-8")
        detail = f"; received={bytes(received)!r}" if "received" in locals() else ""
        print(f"Stage 32.19 live GDB qualification failed: {error}{detail}; evidence: {out}", file=sys.stderr)
        return 1
    finally:
        if client is not None:
            client.close()
    qemu_log.write_text((stdout or b"").decode(errors="replace") + (stderr or b"").decode(errors="replace"), encoding="utf-8")
    log = serial.read_text(encoding="utf-8", errors="replace")
    errors = validation_errors(process.returncode, log, [
        f"[SMP] online={args.cpus} expected={args.cpus}",
        (f"[S32.{20 if args.command == 'D' else 21}] live GDB {'detach' if args.command == 'D' else 'kill'}: PASSED"
         if args.command in ("D", "k") else "[S32.19] live GDB COM2 client qualification: PASSED"),
    ])
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        return 1
    print(f"Stage 32.19 live GDB qualification: PASS ({args.cpus} CPU, exit 33)")
    print(f"Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
