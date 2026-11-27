#!/usr/bin/env python3
"""Stage 14.1 Network validation gate.

Runs the WovenHat kernel with the `network-test` feature and validates
the Stage 14.1 self-tests: DHCP transition, socket stress, and lifecycle.

Usage:
    python scripts/test-stage14-1-network.py --cpus 1 [--firmware QEMU_FD]

The kernel must be built with ``--features network-test stage14-1-test``.
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
        description="Stage 14.1: Network validation gate."
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

    audit_dir = Path.cwd() / "audit-artifacts" / f"stage14.1-{args.cpus}cpu-{int(time.time_ns())}"
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

    # Build the kernel command line; the Cargo project provides a pre-built
    # image via the CI harness.  Here we use the qemu-test infrastructure
    # that already knows how to launch the wovenhat-os image.
    # The image may be in a build cache directory with a hash prefix.
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
        "-net", "user,hostfwd=tcp::5555-:22",
        "-drive", f"if=pflash,format=raw,readonly=on,file={firmware_path}",
        "-drive", f"if=ide,file={kernel_img}",
        "-netdev", "user,id=net0",
        "-device", "e1000,netdev=net0",
    ]

    # Run QEMU and capture exit code; the kernel writes PASS/FAIL lines to
    # stdio and exits 33 on success, 35 on failure.
    try:
        result = subprocess_run(cmd, timeout=args.timeout, capture_output=True, text=True)
        # The kernel prints its own evidence; just forward the exit code.
        exit_code = result.returncode
    except TimeoutExpired:
        print("ERROR: QEMU timed out after {args.timeout}s".format(args=args), file=sys.stderr)
        return 1

    # Write serial output to the audit directory for post-mortem.
    serial_log = audit_dir / "serial.log"
    serial_log.write_text(result.stdout + "\n" + result.stderr)

    if exit_code == 33:
        # Evidence directory with a marker file so the harness can find it.
        (audit_dir / "pass").touch()
        print(
            f"Stage 14.1 network: PASS ({args.cpus} CPU). "
            f"Evidence: {audit_dir}"
        )
        return 0
    else:
        print(
            f"Stage 14.1 network: FAIL (exit {exit_code}). "
            f"Evidence: {audit_dir} (serial.log written)",
            file=sys.stderr,
        )
        return 1


def subprocess_run(*args: object, **kwargs: object) -> object:
    """Minimal shim so the type stub works when `subprocess` is mocked."""
    from subprocess import run as _run
    return _run(*args, **kwargs)


if __name__ == "__main__":
    sys.exit(main())