"""Qualify accepted/rejected boot policy through the actual UEFI runtime.

Use --prepare once to build disposable images, then reuse those exact images
at 1/2/4 CPUs. No host disk, firmware store, or source image is modified.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time

from qemu_validation import validation_errors

ROOT = Path(__file__).resolve().parents[1]
CASES = {
    "default": None,
    "legacy": None,
    "version1": None,
    "malformed": "Malformed",
    "empty": "Malformed",
    "trailing": "TrailingData",
    "unsupported": "UnsupportedVersion",
    "unknown": "Malformed",
    "recovery": None,
    "diagnostics": None,
    "graphics-unsupported": "GraphicsUnsupportedMode",
    "oversized": "TooLarge",
    "kernel-empty": "Empty",
    "kernel-missing": "Missing",
    "kernel-truncated": "InvalidElfOrContract",
    "kernel-corrupt": "InvalidElfOrContract",
    "kernel-wrong-arch": "InvalidElfOrContract",
    "kernel-bad-table": "InvalidElfOrContract",
    "kernel-allocation": "LoaderAllocation",
    "disk-directory": "DiskFileIsDirectory",
    "disk-read": "DiskRead",
}
STOP = "[WBOOT] boot stopped: invalid boot configuration"
BOOTLOADER_SOURCES = (
    "vendor/bootloader/build.rs", "vendor/bootloader/Cargo.toml",
    "vendor/bootloader/uefi/Cargo.toml", "vendor/bootloader/uefi/Cargo.lock",
    "vendor/bootloader/uefi/src/main.rs", "vendor/bootloader/uefi/src/config.rs",
    "vendor/bootloader/uefi/src/memory_descriptor.rs",
    "vendor/bootloader/uefi/src/image.rs",
    "vendor/bootloader-api/src/info.rs",
    "vendor/bootloader-x86_64-common/src/lib.rs",
    "kernel/src/main.rs",
)


def failure_subject(name):
    return "kernel image" if name.startswith("kernel-") else "boot configuration"


def firmware_failure(name):
    return name in ("kernel-allocation", "graphics-unsupported") or name.startswith("disk-")


def stop_marker(name):
    if firmware_failure(name):
        return "[WBOOT] boot stopped: firmware resource failure"
    return f"[WBOOT] boot stopped: invalid {failure_subject(name)}"


def rejection_marker(name, reason):
    if firmware_failure(name):
        return f"[WBOOT] firmware resource failure: {reason}"
    return f"[WBOOT] invalid {failure_subject(name)}: {reason}"


def outcome_errors(name, returncode, log, cpus):
    reason = CASES[name]
    if reason is None:
        return validation_errors(returncode, log, [
            "[BOOT] ALL VALIDATIONS PASSED", f"[SMP] online={cpus} expected={cpus}",
        ])
    errors = []
    if rejection_marker(name, reason) not in log or stop_marker(name) not in log:
        errors.append("missing exact rejection/stop markers")
    if "exiting boot services" in log.lower() or "[BOOT]" in log:
        errors.append("rejected configuration continued to boot handoff")
    if "kernel panic" in log.lower():
        errors.append("unexpected kernel panic")
    if returncode is not None:
        errors.append(f"guest exited instead of remaining in controlled failure: {returncode}")
    return errors


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def regular_input(path, maximum):
    if path.is_symlink() or not path.is_file() or "," in str(path):
        raise ValueError("inputs must be ordinary files without commas in their paths")
    if not 0 < path.stat().st_size <= maximum:
        raise ValueError(f"input size is outside the qualification bound: {path}")


def source_snapshot():
    return {name: hashlib.sha256((ROOT / name).read_bytes().replace(b"\r\n", b"\n")).hexdigest()
            for name in BOOTLOADER_SOURCES}


def run_build(command, log, env):
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
    log.write_text(result.stdout + result.stderr, encoding="utf-8")
    if result.returncode:
        raise RuntimeError(f"build failed with exit {result.returncode}; see {log}")
    return result.stdout


def prepare(fixtures):
    if fixtures.exists() or fixtures.is_symlink():
        raise ValueError("fixture output must not already exist")
    fixtures.parent.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(ROOT / "target")
    env["CARGO_BUILD_BUILD_DIR"] = str(ROOT / ".cargo-build")
    output = run_build(
        ["cargo", "build", "-p", "wovenhat-kernel", "--target", "x86_64-unknown-none",
         "--features", "qemu-test", "--message-format=json"],
        fixtures.with_name(fixtures.name + "-kernel-build.log"), env,
    )
    kernel = None
    for line in output.splitlines():
        record = json.loads(line)
        if (record.get("reason") == "compiler-artifact"
                and record.get("target", {}).get("name") == "wovenhat-kernel"
                and record.get("executable")):
            kernel = Path(record["executable"])
    if kernel is None:
        raise RuntimeError("Cargo did not identify the kernel executable")
    run_build(
        ["cargo", "run", "--manifest-path", "tools/boot-config-fixtures/Cargo.toml",
         "--", str(kernel), str(fixtures)],
        fixtures.with_name(fixtures.name + "-fixture-build.log"), env,
    )
    manifest = {"kernel_sha256": digest(kernel), "source_sha256_lf": source_snapshot(), "images": {
        name: digest(fixtures / f"{name}.img") for name in CASES
    }}
    (fixtures / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")


def qmp(port, execute, arguments=None):
    with socket.create_connection(("127.0.0.1", port), timeout=4) as connection:
        connection.settimeout(4)
        with connection.makefile("rwb") as channel:
            json.loads(channel.readline())
            for request in ({"execute": "qmp_capabilities"},
                            {"execute": execute, "arguments": arguments or {}}):
                channel.write(json.dumps(request).encode() + b"\n")
                channel.flush()
                while True:
                    line = channel.readline()
                    if not line:
                        raise RuntimeError("QMP closed before command completion")
                    response = json.loads(line)
                    if "error" in response:
                        raise RuntimeError(f"QMP rejected {request['execute']}: {response['error']}")
                    if "return" in response:
                        break


def run_case(name, image, qemu, firmware, cpus, timeout, output):
    regular_input(image, 512 * 1024 * 1024)
    regular_input(firmware, 16 * 1024 * 1024)
    serial = output / f"{name}-serial.log"
    serial.touch(exist_ok=False)
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    command = [str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "256M",
               "-smp", str(cpus), "-display", "none", "-vga", "std", "-nic", "none",
               "-serial", f"file:{serial}", "-no-reboot",
               "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
               "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
               "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
               "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
               "-device", "virtio-blk-pci,drive=boot,bootindex=1"]
    (output / f"{name}-command.json").write_text(json.dumps(command), encoding="utf-8")
    with (output / f"{name}-qemu.log").open("w") as errors:
        flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
        process = subprocess.Popen(command, cwd=ROOT, stdout=errors, stderr=errors, creationflags=flags)
        try:
            deadline = time.monotonic() + timeout
            while True:
                log = serial.read_text(errors="replace")
                if process.poll() is not None or (CASES[name] is not None and stop_marker(name) in log):
                    break
                if time.monotonic() >= deadline:
                    raise RuntimeError(f"{name}: timed out; evidence {output}")
                time.sleep(0.05)
            failures = outcome_errors(name, process.poll(), log, cpus)
            if failures:
                raise RuntimeError(f"{name}: {'; '.join(failures)}; evidence {output}")
            if CASES[name] is not None:
                frame = output / f"{name}.ppm"
                qmp(port, "screendump", {"filename": str(frame)})
                regular_input(frame, 64 * 1024 * 1024)
            print(f"Boot config {name}: PASS ({cpus} CPU)", flush=True)
        finally:
            if process.poll() is None:
                try:
                    qmp(port, "quit")
                except (OSError, RuntimeError, ValueError):
                    process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--prepare", action="store_true")
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--qemu", type=Path, default=Path(r"C:\Program Files\qemu\qemu-system-x86_64.exe"))
    parser.add_argument("--firmware", type=Path)
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("timeout must be positive")
    if args.fixtures.is_symlink():
        parser.error("fixtures must not be a symlink")
    fixtures = args.fixtures.resolve()
    if args.prepare:
        prepare(fixtures)
    manifest_path = fixtures / "manifest.json"
    regular_input(manifest_path, 64 * 1024)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("source_sha256_lf") != source_snapshot():
        raise RuntimeError("bootloader source differs from the prepared fixtures; rebuild into a new directory")
    firmware = args.firmware or args.qemu.parent / "share/edk2-x86_64-code.fd"
    regular_input(firmware, 16 * 1024 * 1024)
    firmware = firmware.resolve()
    output = ROOT / "audit-artifacts" / f"boot-config-{args.cpus}cpu-{time.time_ns()}"
    output.mkdir(parents=True, exist_ok=False)
    (output / "provenance.json").write_text(json.dumps({
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "fixtures": manifest, "firmware_sha256": digest(firmware), "cpus": args.cpus,
    }, indent=2), encoding="utf-8")
    for name in CASES:
        image = fixtures / f"{name}.img"
        regular_input(image, 512 * 1024 * 1024)
        if digest(image) != manifest["images"][name]:
            raise RuntimeError(f"fixture changed since preparation: {name}")
        run_case(name, image, args.qemu.resolve(), firmware, args.cpus, args.timeout, output)
    print(f"BOOT CONFIGURATION ACCEPTANCE: PASS; evidence {output}")


if __name__ == "__main__":
    main()
