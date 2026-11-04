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
import importlib.util
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
import zlib
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


def _load_wovenfs_formatter():
    script = ROOT / "scripts" / "format-wovenfs-file-image.py"
    spec = importlib.util.spec_from_file_location("installed_wovenfs_formatter", script)
    if spec is None or spec.loader is None:
        raise InstalledDiskGateError("WovenFS formatter is unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _wovenfs_partition(image: Path) -> tuple[int, int]:
    with image.open("rb") as stream:
        stream.seek(SECTOR)
        header = stream.read(SECTOR)
        if header[:8] != GPT_SIGNATURE:
            raise InstalledDiskGateError("installed image lacks a GPT header")
        entries_lba = int.from_bytes(header[72:80], "little")
        entry_count = int.from_bytes(header[80:84], "little")
        entry_size = int.from_bytes(header[84:88], "little")
        if entry_count == 0 or entry_count > 128 or entry_size < 128 or entry_size > SECTOR:
            raise InstalledDiskGateError("GPT partition array is outside bounded limits")
        stream.seek(entries_lba * SECTOR)
        entries = stream.read(entry_count * entry_size)
    esp_type = bytes.fromhex("28732ac11ff8d211ba4b00a0c93ec93b")
    candidates = []
    for index in range(entry_count):
        entry = entries[index * entry_size:(index + 1) * entry_size]
        if len(entry) < entry_size or entry[:16] == b"\0" * 16:
            continue
        first = int.from_bytes(entry[32:40], "little")
        last = int.from_bytes(entry[40:48], "little")
        if last < first or first == 0:
            continue
        if entry[:16] != esp_type:
            candidates.append((first * SECTOR, (last - first + 1) * SECTOR))
    if not candidates:
        raise InstalledDiskGateError("GPT image has no non-ESP system partition")
    return candidates[0]


def _add_wovenfs_partition(image: Path) -> None:
    old_total = image.stat().st_size // SECTOR
    new_total = old_total + 32768
    first_lba = old_total
    last_lba = new_total - 34
    with image.open("r+b") as stream:
        stream.truncate(new_total * SECTOR)
        stream.seek(2 * SECTOR)
        entries = bytearray(stream.read(128 * 128))
        if len(entries) != 128 * 128:
            raise InstalledDiskGateError("GPT partition array is truncated")
        entries[128:144] = bytes.fromhex("a2a0d0ebe5b9334487c068b6b72699c7")
        entries[144:160] = bytes.fromhex("7b1e3c5a2d9f4f4b9c2d4e6a8b0c1d2e")
        entries[160:176] = first_lba.to_bytes(8, "little") + last_lba.to_bytes(8, "little")
        entries[192:208] = b"WovenFS System".ljust(16, b"\0")
        entries_crc = zlib.crc32(entries) & 0xFFFFFFFF
        stream.seek(2 * SECTOR)
        stream.write(entries)
        stream.seek(SECTOR)
        primary = bytearray(stream.read(SECTOR))
        header_size = int.from_bytes(primary[12:16], "little")
        primary[32:40] = (new_total - 1).to_bytes(8, "little")
        primary[48:56] = last_lba.to_bytes(8, "little")
        primary[88:92] = entries_crc.to_bytes(4, "little")
        primary[16:20] = b"\0" * 4
        primary[16:20] = (zlib.crc32(primary[:header_size]) & 0xFFFFFFFF).to_bytes(4, "little")
        stream.seek(SECTOR)
        stream.write(primary)
        stream.seek((new_total - 33) * SECTOR)
        stream.write(entries)
        backup = bytearray(primary)
        backup[24:32] = (new_total - 1).to_bytes(8, "little")
        backup[32:40] = (1).to_bytes(8, "little")
        backup[72:80] = (new_total - 33).to_bytes(8, "little")
        backup[16:20] = b"\0" * 4
        backup[16:20] = (zlib.crc32(backup[:header_size]) & 0xFFFFFFFF).to_bytes(4, "little")
        stream.seek((new_total - 1) * SECTOR)
        stream.write(backup)
        stream.flush()
        os.fsync(stream.fileno())


def provision_wovenfs_system(image: Path) -> dict[str, object]:
    try:
        offset, size = _wovenfs_partition(image)
    except InstalledDiskGateError:
        _add_wovenfs_partition(image)
        offset, size = _wovenfs_partition(image)
    if size < 16 * SECTOR or size % SECTOR:
        raise InstalledDiskGateError("WovenFS system partition is too small or unaligned")
    formatter = _load_wovenfs_formatter()
    data = formatter._build(size, {"qualification": "installed"})
    with image.open("r+b") as stream:
        stream.seek(offset)
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    return {"filesystem": "wovenfs", "offset": offset, "bytes": size, "sha256": hashlib.sha256(data).hexdigest(), "physical_device_access": "none"}


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


def install_exact_disk_image(source: Path, destination: Path, *, filesystem: str = "fat32") -> dict[str, object]:
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
    provisioning = None
    if filesystem == "wovenfs":
        provisioning = provision_wovenfs_system(target)
        installed = validate_source_image(target)
    elif filesystem != "fat32":
        raise InstalledDiskGateError(f"unsupported installed filesystem: {filesystem}")
    if filesystem == "fat32" and installed["sha256"] != source_info["sha256"]:
        raise InstalledDiskGateError("installed disk digest differs from source image")
    return {
        "source": source_info,
        "installed": installed,
        "exact_digest_match": filesystem == "fat32",
        "install_mode": (
            "atomic-file-backed-exact-uefi-image-copy"
            if filesystem == "fat32"
            else "atomic-file-backed-uefi-image-copy-with-wovenfs-system-provisioning"
        ),
        "provisioning": provisioning,
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
    filesystem: str = "fat32",
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
        "pc" if filesystem == "wovenfs" else "q35",
        "-m",
        "2G",
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
    drive = f"if=none,id=installed,format=raw,file={installed_disk}"
    if filesystem == "fat32":
        drive = f"if=none,id=installed,format=raw,readonly=on,file={installed_disk}"
    if filesystem == "wovenfs":
        command.extend(
            [
                "-drive",
                f"if=none,id=installed,format=raw,file={installed_disk}",
                "-device",
                "ide-hd,bus=ide.0,drive=installed",
            ]
        )
    else:
        command.extend(
            [
                "-drive",
                drive,
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
    filesystem: str,
) -> dict[str, object]:
    profile = output_root / f"{cpus}cpu"
    profile.mkdir(parents=True, exist_ok=False)
    installed = profile / "installed-disk.img"
    serial = profile / "serial.log"
    qemu_log = profile / "qemu.log"
    qmp = profile / "qmp.sock"
    install = install_exact_disk_image(source, installed, filesystem=filesystem)
    vars_copy = None
    if firmware_vars is not None:
        vars_copy = _copy_vars(firmware_vars, profile / "OVMF_VARS.fd")
    command = build_qemu_command(qemu, firmware, vars_copy, installed, serial, qmp, cpus, filesystem)
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
    filesystem: str,
) -> dict[str, object]:
    source_info = validate_source_image(source)
    if DEVICE_PATH_RE.search(str(output_root)):
        raise InstalledDiskGateError(f"output directory is a physical/device path: {output_root}")
    if output_root.exists():
        raise InstalledDiskGateError(f"refusing to overwrite existing output directory: {output_root}")
    output_root.mkdir(parents=True, exist_ok=False)
    results = []
    for profile in cpus:
        results.append(run_profile(source_info_path(source_info), qemu, firmware, firmware_vars, output_root, profile, timeout, filesystem))
    passed = all(bool(item["guest"]["guest_execution"]) for item in results)
    result = {
        "schema": "woven-installed-disk-qemu-v1",
        "valid": passed,
        "host_preparation": {"source": source_info, "physical_device_access": "none"},
        "guest_execution": passed,
        "media_attached": False,
        "filesystem": filesystem,
        "profiles": results,
    }
    (output_root / "result.json").write_text(json_dumps(result) + "\n", encoding="utf-8")
    return result


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel-image", type=Path, help="existing complete UEFI disk image; otherwise build it")
    parser.add_argument("--feature", default="stage28-installer-test")
    parser.add_argument("--qemu", type=Path)
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--firmware-vars", type=Path)
    parser.add_argument("--filesystem", choices=("fat32", "wovenfs"), default="fat32")
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
            filesystem=args.filesystem,
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
