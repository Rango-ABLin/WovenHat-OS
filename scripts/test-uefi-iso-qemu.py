"""Boot one exact WovenHat UEFI ISO in QEMU and preserve serial evidence.

This gate uses only a regular ISO file and a caller-supplied regular OVMF code
file. It never opens a physical disk and never terminates unrelated processes.
QEMU is allowed to run until the timeout because the kernel normally remains
interactive; required serial markers establish that firmware and the kernel
were reached. A later kernel self-test failure remains a gate failure.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


PROTECTED_RE = re.compile(
    r"(?i)(removable|usb|backup|external|physicaldrive|/dev/|\\\\[.?]\\)"
)
DEFAULT_MARKERS = ("UEFI bootloader started", "TRACE: exiting boot services")


def _regular(path: Path, label: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"{label} must be a regular non-symlink file: {path}")
    if PROTECTED_RE.search(str(path)):
        raise ValueError(f"{label} path is protected or device-like: {path}")


def run_gate(iso: Path, firmware: Path, qemu: Path, serial_log: Path,
             timeout: float, markers: tuple[str, ...]) -> dict[str, object]:
    _regular(iso, "ISO")
    _regular(firmware, "OVMF firmware")
    _regular(qemu, "QEMU")
    if serial_log.exists() and serial_log.is_symlink():
        raise ValueError("serial log must not be a symlink")
    serial_log.parent.mkdir(parents=True, exist_ok=True)
    command = [
        str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "256M",
        "-smp", "1", "-display", "none", "-serial", f"file:{serial_log}",
        "-no-reboot", "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
        "-cdrom", str(iso), "-boot", "d",
    ]
    timed_out = False
    return_code: int | None = None
    process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    try:
        _, stderr = process.communicate(timeout=timeout)
        return_code = process.returncode
    except subprocess.TimeoutExpired:
        timed_out = True
        process.terminate()
        try:
            _, stderr = process.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            _, stderr = process.communicate()
    serial = serial_log.read_text(encoding="utf-8", errors="replace") if serial_log.exists() else ""
    found = {marker: marker in serial for marker in markers}
    forbidden = [line for line in serial.splitlines() if " self-test FAIL" in line or "FAIL" in line]
    passed = all(found.values()) and not forbidden
    result = {
        "iso": str(iso.resolve()),
        "firmware": str(firmware.resolve()),
        "qemu": str(qemu.resolve()),
        "serial_log": str(serial_log.resolve()),
        "required_markers": found,
        "forbidden_failure_lines": forbidden,
        "qemu_returncode": return_code,
        "timed_out_after_seconds": timeout if timed_out else None,
        "passed": passed,
        "physical_disk_access": False,
        "command": command,
        "stderr": stderr[-4000:],
    }
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--iso", type=Path, required=True)
    parser.add_argument("--firmware", type=Path, required=True)
    parser.add_argument("--qemu", type=Path, required=True)
    parser.add_argument("--serial-log", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--marker", action="append", dest="markers")
    args = parser.parse_args()
    try:
        result = run_gate(args.iso.resolve(), args.firmware.resolve(), args.qemu.resolve(),
                          args.serial_log.resolve(), args.timeout,
                          tuple(args.markers or DEFAULT_MARKERS))
    except (OSError, ValueError) as exc:
        parser.error(str(exc))
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
