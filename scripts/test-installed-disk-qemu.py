#!/usr/bin/env python3
"""Qualify boot from the exact disposable installed disk image.

The source is a complete WovenHat UEFI disk image produced by the existing
Cargo/bootloader image path.  The install step is the file-backed equivalent
of the disk installer: it copies that already-partitioned GPT/FAT32 image to
an atomically-created disposable destination and verifies the destination
digest.  QEMU is then given only the installed destination.  No installer
media, host disk, device path, mount, reboot, or firmware source is modified.

This gate distinguishes host preparation from guest execution.  A valid GPT
header or a successful file copy never counts as boot success; the exact
installed image must reach the UEFI and kernel serial markers and exit through
the guest debug-exit device.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
from typing import Iterable, Sequence


ROOT = Path(__file__).resolve().parents[1]
SECTOR = 512
GPT_SIGNATURE = b"EFI PART"
GUEST_EXIT_SUCCESS = 33
UEFI_MARKER = "INFO : UEFI bootloader started"
KERNEL_MARKER = "[BOOT] ALL VALIDATIONS PASSED"
DEVICE_PATH_RE = re.compile(
    r"(?i)(?:^|[/\\])dev[/\\]|(?:^|[/\\])physicaldrive\d*$|\\\\[.?]\\physicaldrive"
)


class InstalledDiskGateError(RuntimeError):
    """A fail-closed installed-disk gate error."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def regular_file(path: Path, label: str, *, must_exist: bool = True) -> Path:
    """Return a safe regular file and reject device-like paths/symlinks."""
    candidate = Path(path)
    if DEVICE_PATH_RE.search(str(candidate)):
        raise InstalledDiskGateError(f"{label} is a physical/device path: {candidate}")
    if not must_exist:
        return candidate.absolute()
    try:
        stat = candidate.lstat()
    except OSError as exc:
        raise InstalledDiskGateError(f"{label} is not readable: {candidate}") from exc
    if candidate.is_symlink() or not candidate.is_file() or not stat:
        raise InstalledDiskGateError(f"{label} must be an existing regular non-symlink file: {candidate}")
    return candidate.absolute()


def validate_source_image(path: Path) -> dict[str, object]:
    """Validate the source as a file-backed GPT UEFI disk image only."""
    image = regular_file(path, "kernel disk image")
    size = image.stat().st_size
    if size < 32 * 1024 * 1024 or size % SECTOR:
        raise InstalledDiskGateError(
            f"kernel disk image is not a bounded sector-aligned image: {size} bytes"
        )
    with image.open("rb") as stream:
        mbr = stream.read(SECTOR)
        gpt = stream.read(SECTOR)
    if mbr[510:512] != b"\x55\xaa":
        raise InstalledDiskGateError("kernel disk image lacks a protective MBR")
    if gpt[: len(GPT_SIGNATURE)] != GPT_SIGNATURE:
        raise InstalledDiskGateError("kernel disk image lacks a GPT header")
    return {
        "path": str(image),
        "bytes": size,
        "sha256": sha256(image),
        "protective_mbr": True,
        "gpt_header": True,
    }


def install_exact_disk_image(source: Path, destination: Path) -> dict[str, object]:
    """Atomically install a complete file-backed UEFI image.

    This is deliberately an image-level equivalent, not a raw-device writer.
    The source image already contains the GPT, FAT32 ESP, UEFI loader, and
    embedded kernel produced by the reviewed Cargo boot-image path.  The
    destination is newly created in the evidence directory and is the only
    disk passed to QEMU.
    """
    source_info = validate_source_image(source)
    target = regular_file(destination, "installed disk", must_exist=False)
    if target.exists() or target.is_symlink():
        raise InstalledDiskGateError(f"refusing to overwrite installed disk: {target}")
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            prefix=f".{target.name}.", suffix=".partial", dir=target.parent, delete=False
        ) as stream:
            temporary = Path(stream.name)
            with source_info_path(source_info).open("rb") as input_stream:
                shutil.copyfileobj(input_stream, stream, length=1024 * 1024)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, target)
        temporary = None
    finally:
        if temporary is not None:
            try:
                temporary.unlink()
            except FileNotFoundError:
                pass
    installed = validate_source_image(target)
    if installed["sha256"] != source_info["sha256"]:
        raise InstalledDiskGateError("installed disk digest differs from source image")
    return {
        "source": source_info,
        "installed": installed,
        "exact_digest_match": True,
        "install_mode": "atomic-file-backed-exact-uefi-image-copy",
        "physical_device_access": "none",
    }


def source_info_path(info: dict[str, object]) -> Path:
    value = info.get("path")
    if not isinstance(value, str):
        raise InstalledDiskGateError("source image metadata lost its path")
    return Path(value)


def build_qemu_command(
    qemu: Path,
    firmware: Path,
    firmware_vars: Path | None,
    installed_disk: Path,
    serial: Path,
    qmp: Path,
    cpus: int,
) -> list[str]:
    """Build a QEMU command containing only the installed disk as storage."""
    if cpus not in (1, 2, 4):
        raise InstalledDiskGateError(f"unsupported CPU profile: {cpus}")
    qemu = regular_file(qemu, "QEMU")
    firmware = regular_file(firmware, "OVMF CODE")
    installed_disk = regular_file(installed_disk, "installed disk")
    command = [
        str(qemu),
        "-accel",
        "tcg,tb-size=128",
        "-machine",
        "q35",
        "-m",
        "4G",
        "-smp",
        str(cpus),
        "-display",
        "none",
        "-serial",
        f"file:{serial}",
        "-qmp",
        f"unix:{qmp},server=on,wait=off",
        "-no-reboot",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-drive",
        f"if=pflash,format=raw,readonly=on,file={firmware}",
    ]
    if firmware_vars is not None:
        command.extend(["-drive", f"if=pflash,format=raw,file={firmware_vars}"])
    command.extend(
        [
            "-drive",
            f"if=none,id=installed,format=raw,readonly=on,file={installed_disk}",
            "-device",
            "virtio-blk-pci,drive=installed,bootindex=1",
        ]
    )
    return command


def guest_execution_status(exit_code: int, serial_text: str) -> dict[str, object]:
    """Classify actual guest evidence without accepting host-only preparation."""
    uefi = UEFI_MARKER in serial_text
    kernel = KERNEL_MARKER in serial_text
    debug_exit = exit_code == GUEST_EXIT_SUCCESS
    return {
        "guest_execution": bool(uefi and kernel and debug_exit),
        "uefi_bootloader_started": uefi,
        "kernel_validation_passed": kernel,
        "guest_debug_exit": debug_exit,
        "qemu_exit_code": exit_code,
    }


def _copy_vars(template: Path, destination: Path) -> Path:
    source = regular_file(template, "OVMF VARS template")
    with source.open("rb") as input_stream, destination.open("xb") as output_stream:
        shutil.copyfileobj(input_stream, output_stream)
        output_stream.flush()
        os.fsync(output_stream.fileno())
    return destination


def resolve_qemu_and_firmware(qemu: Path | None, firmware: Path | None) -> tuple[Path, Path]:
    resolved_qemu = qemu or (Path(shutil.which("qemu-system-x86_64")) if shutil.which("qemu-system-x86_64") else None)
    if resolved_qemu is None:
        raise InstalledDiskGateError("QEMU is unavailable; provide --qemu")
    resolved_qemu = regular_file(resolved_qemu, "QEMU")
    resolved_firmware = firmware or resolved_qemu.parent / "share" / "edk2-x86_64-code.fd"
    resolved_firmware = regular_file(resolved_firmware, "OVMF CODE")
    return resolved_qemu, resolved_firmware


def build_kernel_image(root: Path, feature: str, timeout: float) -> Path:
    """Build through the existing Cargo UEFI image path and return its image."""
    env = dict(os.environ)
    env.setdefault("CARGO_BUILD_BUILD_DIR", str(root / ".cargo-build"))
    env.setdefault("CARGO_TARGET_DIR", str(root / "target"))
    command = ["cargo", "run", "--quiet", "--features", feature, "--", "--print-image"]
    try:
        result = subprocess.run(
            command,
            cwd=root,
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        raise InstalledDiskGateError("Cargo UEFI image build timed out") from exc
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()[-2000:]
        raise InstalledDiskGateError(f"Cargo UEFI image build failed: {detail}")
    candidates = [Path(line.strip()) for line in result.stdout.splitlines() if line.strip().endswith(".img")]
    for candidate in reversed(candidates):
        if candidate.is_file() and not candidate.is_symlink():
            return candidate.absolute()
    raise InstalledDiskGateError(
        "Cargo completed but did not emit a regular UEFI disk image path; "
        "the current kernel artifact cannot be installed by this gate"
    )


def run_profile(
    source: Path,
    qemu: Path,
    firmware: Path,
    firmware_vars: Path | None,
    output_root: Path,
    cpus: int,
    timeout: float,
) -> dict[str, object]:
    profile = output_root / f"{cpus}cpu"
    profile.mkdir(parents=True, exist_ok=False)
    installed = profile / "installed-disk.img"
    serial = profile / "serial.log"
    qemu_log = profile / "qemu.log"
    qmp = profile / "qmp.sock"
    install = install_exact_disk_image(source, installed)
    vars_copy = None
    if firmware_vars is not None:
        vars_copy = _copy_vars(firmware_vars, profile / "OVMF_VARS.fd")
    command = build_qemu_command(qemu, firmware, vars_copy, installed, serial, qmp, cpus)
    started = time.monotonic()
    timeout_error = None
    try:
        with qemu_log.open("w", encoding="utf-8") as log:
            completed = subprocess.run(
                command,
                cwd=ROOT,
                stdout=log,
                stderr=subprocess.STDOUT,
                timeout=timeout,
                check=False,
            )
    except subprocess.TimeoutExpired as exc:
        timeout_error = str(exc)
        completed = None
    serial_text = serial.read_text(encoding="utf-8", errors="replace") if serial.exists() else ""
    status = guest_execution_status(-1 if completed is None else completed.returncode, serial_text)
    result = {
        "cpus": cpus,
        "install": install,
        "guest": status,
        "media_attached": False,
        "installed_disk": str(installed),
        "serial": str(serial),
        "qemu_log": str(qemu_log),
        "command": command,
        "elapsed_seconds": round(time.monotonic() - started, 3),
        "physical_device_access": "none",
    }
    if timeout_error is not None:
        result["error"] = (
            f"{cpus}-CPU installed-disk guest timed out; the preserved serial log "
            f"did not prove terminal guest completion: {serial}"
        )
    elif not status["guest_execution"]:
        missing = []
        if not status["uefi_bootloader_started"]:
            missing.append(UEFI_MARKER)
        if not status["kernel_validation_passed"]:
            missing.append(KERNEL_MARKER)
        if not status["guest_debug_exit"]:
            missing.append(f"QEMU exit {GUEST_EXIT_SUCCESS}")
        result["error"] = (
            f"{cpus}-CPU installed-disk image did not boot as a guest; missing "
            f"{', '.join(missing)}; serial evidence: {serial}; qemu log: {qemu_log}"
        )
    (profile / "result.json").write_text(json_dumps(result), encoding="utf-8")
    return result


def run_gate(
    *,
    source: Path,
    qemu: Path,
    firmware: Path,
    firmware_vars: Path | None,
    output_root: Path,
    cpus: Iterable[int],
    timeout: float,
) -> dict[str, object]:
    source_info = validate_source_image(source)
    if DEVICE_PATH_RE.search(str(output_root)):
        raise InstalledDiskGateError(f"output directory is a physical/device path: {output_root}")
    if output_root.exists():
        raise InstalledDiskGateError(f"refusing to overwrite existing output directory: {output_root}")
    output_root.mkdir(parents=True, exist_ok=False)
    results = []
    for profile in cpus:
        results.append(run_profile(source_info_path(source_info), qemu, firmware, firmware_vars, output_root, profile, timeout))
    passed = all(bool(item["guest"]["guest_execution"]) for item in results)
    return {
        "schema": "woven-installed-disk-qemu-v1",
        "valid": passed,
        "host_preparation": {"source": source_info, "physical_device_access": "none"},
        "guest_execution": passed,
        "media_attached": False,
        "profiles": results,
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel-image", type=Path, help="existing complete UEFI disk image; otherwise build it")
    parser.add_argument("--feature", default="stage28-installer-test")
    parser.add_argument("--qemu", type=Path)
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--firmware-vars", type=Path)
    cpu_group = parser.add_mutually_exclusive_group()
    cpu_group.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    cpu_group.add_argument("--all-cpus", action="store_true")
    parser.add_argument("--timeout", type=float, default=300.0)
    parser.add_argument("--output-dir", type=Path)
    args = parser.parse_args(argv)
    try:
        qemu, firmware = resolve_qemu_and_firmware(args.qemu, args.firmware)
        source = args.kernel_image or build_kernel_image(ROOT, args.feature, args.timeout)
        source = validate_source_image(source)["path"]
        source_path = Path(source)
        timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        output = (args.output_dir or ROOT / "target" / f"installed-disk-release-{timestamp}").absolute()
        profiles = (1, 2, 4) if args.all_cpus else (args.cpus,)
        result = run_gate(
            source=source_path,
            qemu=qemu,
            firmware=firmware,
            firmware_vars=args.firmware_vars,
            output_root=output,
            cpus=profiles,
            timeout=args.timeout,
        )
    except (InstalledDiskGateError, OSError, ValueError) as exc:
        print(f"INSTALLED-DISK-RELEASE: FAIL: {exc}", file=sys.stderr)
        return 2
    print(json_dumps(result))
    return 0 if result["valid"] else 1


def json_dumps(value: object) -> str:
    import json

    return json.dumps(value, indent=2, sort_keys=True)


if __name__ == "__main__":
    raise SystemExit(main())
