"""Stage 20.14S: one virtio-GPU output through the production compositor.

The kernel renders four quadrant colours through the single-output GPU path;
QMP captures the host scanout and this harness checks independent samples.
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


READY = "[S20.14S] ready for capture"
PASS = "[S20.14S] single-output GPU: PASSED"
COLOURS = ((208, 64, 64), (64, 208, 112), (64, 112, 224), (224, 208, 64))


def free_port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def read_ppm(path):
    data = path.read_bytes()
    parts = []
    index = 0
    while len(parts) < 4:
        while data[index:index + 1].isspace():
            index += 1
        start = index
        while data[index:index + 1] != b" " and not data[index:index + 1].isspace():
            index += 1
        parts.append(data[start:index])
    index += 1
    if parts[0] != b"P6" or parts[3] != b"255":
        raise ValueError(f"unexpected PPM header {parts}")
    return int(parts[1]), int(parts[2]), data[index:]


def qmp_session(port, deadline):
    while True:
        try:
            sock = socket.create_connection(("127.0.0.1", port), timeout=10)
            break
        except OSError:
            if time.time() > deadline:
                raise
            time.sleep(0.2)
    stream = sock.makefile("rw", encoding="utf-8", newline="\n")
    stream.readline()

    def command(name, **arguments):
        message = {"execute": name}
        if arguments:
            message["arguments"] = arguments
        stream.write(json.dumps(message) + "\n")
        stream.flush()
        while True:
            reply = json.loads(stream.readline())
            if "return" in reply:
                return reply["return"]
            if "error" in reply:
                raise RuntimeError(f"{name}: {reply['error']}")

    command("qmp_capabilities")
    return sock, command


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--timeout", type=float, default=300)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("QEMU and firmware files must exist")
    out = root / "audit-artifacts" / f"stage20.14s-{args.cpus}cpu-{time.time_ns()}"
    out.mkdir(parents=True)
    build = subprocess.run(
        ["cargo", "run", "--quiet", "--features", "stage20-14-single-test", "--", "--print-image"],
        cwd=root, text=True, capture_output=True,
    )
    (out / "build.log").write_text(build.stdout + build.stderr, encoding="utf-8")
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    serial = out / "serial.log"
    port = free_port()
    command = [str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "256M",
               "-smp", str(args.cpus), "-display", "none", "-serial", f"file:{serial}",
               "-no-reboot", "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
               "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
               "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
               "-drive", f"if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}",
               "-device", "virtio-blk-pci,drive=boot,bootindex=1", "-vga", "std",
               "-device", "virtio-gpu-pci,id=gpu0,max_outputs=1"]
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    errors = []
    captured = False
    with (out / "qemu.log").open("w") as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        started = time.time()
        try:
            sock, qmp = qmp_session(port, started + 30)
            while process.poll() is None:
                if time.time() - started > args.timeout:
                    errors.append("timeout")
                    break
                log = serial.read_text(errors="replace") if serial.exists() else ""
                if not captured and READY in log:
                    captured = True
                    qmp("screendump", filename=str(out / "head0.ppm"), device="gpu0", head=0)
                time.sleep(0.2)
            sock.close()
        except Exception as error:  # noqa: BLE001 - recorded as a failure
            errors.append(f"QMP: {error}")
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
    log = serial.read_text(errors="replace") if serial.exists() else ""
    if process.returncode != 33:
        errors.append(f"exit code {process.returncode}, expected 33")
    if PASS not in log:
        errors.append(f"missing {PASS!r}")
    if not captured or not (out / "head0.ppm").exists():
        errors.append("single-output scanout was not captured")
    else:
        width, height, pixels = read_ppm(out / "head0.ppm")
        for x, y, expected in ((width // 4, height // 4, COLOURS[0]),
                               (3 * width // 4, height // 4, COLOURS[1]),
                               (width // 4, 3 * height // 4, COLOURS[2]),
                               (3 * width // 4, 3 * height // 4, COLOURS[3])):
            offset = (y * width + x) * 3
            got = tuple(pixels[offset:offset + 3])
            if got != expected:
                errors.append(f"pixel ({x},{y}) captured {got}, expected {expected}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print("FAILED; evidence:", out, file=sys.stderr)
        return 1
    print(f"Stage 20.14S: PASS ({args.cpus} CPU, single-output pixels verified). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
