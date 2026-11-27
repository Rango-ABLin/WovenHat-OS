#!/usr/bin/env python3
"""Stage 15.3 Sandbox capability boundary validation gate.

Runs the WovenHat kernel and validates sandbox capability ID generation,
claim/revoke operations, and generation-tag reuse prevention.

Usage:
    python scripts/test-stage15-3-sandbox.py --cpus 1 [--firmware QEMU_FD]

The kernel must be built with the default host profile (no special features
required; sandbox logic is always compiled but validated at runtime).
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

try:
    from subprocess import TimeoutExpired  # noqa: F401
except ImportError:  # pragma: no cover
    TimeoutExpired = Exception  # type: ignore[misc]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Stage 15.3: Sandbox capability boundary validation gate."
    )
    parser.add_argument(
        "--cpus",
        type=int,
        choices=(1, 2, 4),
        default=1,
        help="Number of CPUs for the QEMU guest (default: 1)",
    )
    parser.add_argument(
        "--firmware",
        type=Path,
        default=None,
        help="Path to OVMF firmware FD file (default: check common paths)",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=180,
        help="Maximum seconds to wait for QEMU to exit (default: 180)",
    )
    args = parser.parse_args(argv)

    # Resolve firmware
    firmware_path: Path | None = args.firmware
    if firmware_path is None:
        for candidate in [
            Path("/usr/share/qemu/edk2-x86_64-code.fd"),
            Path("/usr/share/ovmf/edk2-x86_64-code.fd"),
            Path.cwd() / "edk2-x86_64-code.fd",
            Path.cwd() / "OVMF.fd",
        ]:
            if candidate.is_file():
                firmware_path = candidate
                break
        if firmware_path is None:
            print(
                "ERROR: No OVMF firmware FD found. "
                "Pass --firmware <path> to continue.",
                file=sys.stderr,
            )
            return 1

    audit_dir = Path.cwd() / "audit-artifacts" / f"stage15.3-{args.cpus}cpu-{int(time.time_ns())}"
    audit_dir.mkdir(parents=True, exist_ok=True)

    qemu_path = Path("/usr/bin/qemu-system-x86_64")
    if not qemu_path.is_file():
        # Windows fallback
        win_qemu = Path("C:/Program Files/qemu/qemu-system-x86_64.exe")
        if win_qemu.is_file():
            qemu_path = win_qemu
        else:
            print(f"ERROR: QEMU not found at {qemu_path} or {win_qemu}", file=sys.stderr)
            return 1

    # Kernel image produced by the build; may be in build cache with a hash prefix.
    kernel_img: Path | None = None
    build_dir = Path.cwd() / "target" / "debug" / "build"
    if build_dir.is_dir():
        for entry in build_dir.rglob("wovenhat-os-uefi.img"):
            kernel_img = entry
            break
    if kernel_img is None or not kernel_img.is_file():
        kernel_img = Path.cwd() / "target" / "debug" / "wovenhat-os-uefi.img"
        if not kernel_img.is_file():
            kernel_img = Path.cwd() / "target" / "debug" / "wovenhat-os.img"

    cmd = [
        str(qemu_path),
        "-accel", "tcg,tb-size=128",
        "-machine", "q35",
        "-m", "1G",
        "-smp", str(args.cpus),
        "-serial", "stdio",
        "-drive", f"if=pflash,format=raw,readonly=on,file={firmware_path}",
        "-drive", f"if=ide,file={kernel_img}",
    ]

    try:
        from subprocess import run as _run
        result = _run(cmd, timeout=args.timeout, capture_output=True, text=True)
        exit_code = result.returncode
    except TimeoutExpired:
        print(
            "ERROR: QEMU timed out after {args.timeout}s".format(args=args),
            file=sys.stderr,
        )
        return 1

    serial_log = audit_dir / "serial.log"
    serial_log.write_text(result.stdout + "\n" + result.stderr)

    if exit_code == 33:
        (audit_dir / "pass").touch()
        print(
            f"Stage 15.3 Sandbox capabilities: PASS ({args.cpus} CPU). "
            f"Evidence: {audit_dir}"
        )
        return 0
    else:
        print(
            f"Stage 15.3 Sandbox capabilities: FAIL (exit {exit_code}). "
            f"Evidence: {audit_dir} (serial.log written)",
            file=sys.stderr,
        )
        return 1


if __name__ == "__main__":
    sys.exit(main())