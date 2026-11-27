"""Run the disposable real PXE TFTP-read-failure acceptance fixture."""
import argparse
import json
from pathlib import Path
import socket
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]

def file_input(path, maximum):
    path = path.resolve()
    if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= maximum:
        raise ValueError(f"invalid regular-file input: {path}")
    return path

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", type=Path, required=True)
    parser.add_argument("--ovmf-code", type=Path, required=True)
    parser.add_argument("--ovmf-vars-template", type=Path, required=True)
    parser.add_argument("--shell-media", type=Path, required=True)
    parser.add_argument("--tftp-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--timeout", type=float, default=90)
    args = parser.parse_args()
    if args.output.exists() or args.timeout <= 0:
        parser.error("output must be new and timeout must be positive")
    qemu = file_input(args.qemu, 128 * 1024 * 1024)
    code = file_input(args.ovmf_code, 16 * 1024 * 1024)
    vars_template = file_input(args.ovmf_vars_template, 16 * 1024 * 1024)
    shell = file_input(args.shell_media, 512 * 1024 * 1024)
    root = args.tftp_root.resolve()
    bootloader = root / "bootloader"
    file_input(bootloader, 64 * 1024 * 1024)
    args.output.mkdir(parents=True)
    vars_copy = args.output / "OVMF_VARS.fd"
    vars_copy.write_bytes(vars_template.read_bytes())
    serial, server_log, qemu_log = (args.output / "serial.log", args.output / "server.log", args.output / "qemu.log")
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0)); port = probe.getsockname()[1]
    command = [str(qemu), "-accel", "tcg", "-machine", "q35", "-m", "256M", "-smp", str(args.cpus),
        "-display", "none", "-serial", f"file:{serial}", "-no-reboot", "-boot", "once=n",
        "-fw_cfg", "name=opt/org.tianocore/VirtioNetSupport,string=yes",
        "-fw_cfg", "name=opt/org.tianocore/IPv4PXESupport,string=yes",
        "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={code}",
        "-drive", f"if=pflash,unit=1,format=raw,file={vars_copy}",
        "-device", "virtio-rng-pci",
        "-netdev", f"socket,id=net0,listen=127.0.0.1:{port}",
        "-device", "virtio-net-pci,netdev=net0,bootindex=1"]
    (args.output / "command.json").write_text(json.dumps(command, indent=2), encoding="utf-8")
    flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
    with qemu_log.open("w", encoding="utf-8") as errors:
        qemu_process = subprocess.Popen(command, cwd=ROOT, stdout=errors, stderr=errors, creationflags=flags)
        server = subprocess.Popen([sys.executable, str(ROOT / "scripts" / "tftp-read-failure-server.py"),
            "--connect", f"127.0.0.1:{port}", "--tftp-root", str(root), "--log", str(server_log)],
            cwd=ROOT, creationflags=flags)
        try:
            deadline = time.monotonic() + args.timeout
            while time.monotonic() < deadline:
                log = serial.read_text(errors="replace") if serial.exists() else ""
                events = server_log.read_text(errors="replace") if server_log.exists() else ""
                if "[WBOOT] firmware resource failure: TftpRead" in log and "TFTP kernel tsize acknowledged" in events and "TFTP kernel read deliberately rejected" in events:
                    print(f"TFTP read fixture: PASS ({args.cpus} CPU); evidence {args.output}")
                    return
                if qemu_process.poll() is not None:
                    raise RuntimeError(f"QEMU exited early: {qemu_process.returncode}")
                if server.poll() is not None:
                    raise RuntimeError(f"fixture server exited early: {server.returncode}")
                time.sleep(0.05)
            raise RuntimeError(f"fixture timed out; evidence {args.output}")
        finally:
            for process in (server, qemu_process):
                if process.poll() is None:
                    process.terminate()
                    try: process.wait(timeout=5)
                    except subprocess.TimeoutExpired: process.kill()

if __name__ == "__main__":
    main()
