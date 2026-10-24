#!/usr/bin/env python3
"""Produce a read-only, fail-closed physical qualification preflight report.

This tool inventories the host using Python's standard library only.  It never
opens disks, enumerates device handles, writes media, changes firmware,
reboots, reads credentials, or certifies hardware.  Native qualification is
accepted only by a separate signed evidence workflow outside this preflight.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import sys
from pathlib import Path
from typing import Any


SCHEMA = "wovenhat.hardware-qualification-preflight/v1"
REPORT_NAME = "report.json"
QUALIFICATION_STATUSES = {"BLOCKED", "PHYSICAL_NOT_TESTED"}
CATEGORIES = (
    "intel-pc", "amd-pc", "laptop", "desktop", "nvme", "sata", "usb",
    "ethernet", "wifi", "audio", "intel-graphics", "amd-graphics",
    "nvidia-graphics", "touchpad", "webcam", "bluetooth", "multi-monitor",
)

# These patterns are deliberately conservative.  A preflight must not create
# an output directory that could be interpreted as a raw-device target.
_PHYSICAL_PATH_PATTERNS = (
    re.compile(r"(?:^|[\\/])\\\\\.\\physicaldrive\d+(?:$|[\\/])", re.I),
    re.compile(r"(?:^|[\\/])physicaldrive\d+(?:$|[\\/])", re.I),
    re.compile(r"(?:^|[\\/])dev[\\/]sd[a-z]\d*(?:$|[\\/])", re.I),
    re.compile(r"(?:^|[\\/])dev[\\/]nvme\d+n\d+(?:p\d+)?(?:$|[\\/])", re.I),
    re.compile(r"(?:^|[\\/])dev[\\/]disk(?:$|[\\/])", re.I),
    re.compile(r"(?:^|[\\/])raw(?:[-_]?disk|[-_]?device)(?:$|[\\/])", re.I),
)


def is_safe_output_path(value: str | os.PathLike[str]) -> bool:
    """Return false for paths resembling raw physical-device targets."""

    text = os.fspath(value).replace("/", "\\")
    normalized = "\\" + text.lstrip("\\")
    return not any(pattern.search(normalized) for pattern in _PHYSICAL_PATH_PATTERNS)


def _ram_bytes() -> int | None:
    """Read total RAM without opening a device or invoking a shell command."""

    if hasattr(os, "sysconf"):
        try:
            pages = os.sysconf("SC_PHYS_PAGES")
            size = os.sysconf("SC_PAGE_SIZE")
            if isinstance(pages, int) and isinstance(size, int) and pages > 0 and size > 0:
                return pages * size
        except (OSError, ValueError, TypeError):
            pass
    if sys.platform == "win32":
        try:
            import ctypes

            class MemoryStatus(ctypes.Structure):
                _fields_ = [("length", ctypes.c_uint32), ("memory_load", ctypes.c_uint32),
                            ("total_phys", ctypes.c_uint64), ("available_phys", ctypes.c_uint64),
                            ("total_page_file", ctypes.c_uint64), ("available_page_file", ctypes.c_uint64),
                            ("total_virtual", ctypes.c_uint64), ("available_virtual", ctypes.c_uint64),
                            ("available_extended", ctypes.c_uint64)]

            status = MemoryStatus()
            status.length = ctypes.sizeof(MemoryStatus)
            if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)):
                return int(status.total_phys)
        except (AttributeError, OSError, TypeError):
            pass
    return None


def collect_host_inventory() -> dict[str, Any]:
    """Collect identity and capacity facts using stdlib APIs only."""

    return {
        "host_id": platform.node() or "unknown-host",
        "system": platform.system() or "unknown",
        "release": platform.release() or "unknown",
        "version": platform.version() or "unknown",
        "machine": platform.machine() or "unknown",
        "processor": platform.processor() or "unknown",
        "python": platform.python_version(),
        "logical_cpu_count": os.cpu_count(),
        "ram_bytes": _ram_bytes(),
        "collection": {
            "method": "python-stdlib-only",
            "commands_executed": [],
            "device_handles_opened": False,
            "raw_disks_opened": False,
            "credentials_accessed": False,
        },
    }


def build_report() -> dict[str, Any]:
    host = collect_host_inventory()
    return {
        "schema": SCHEMA,
        "report_kind": "read-only-physical-qualification-preflight",
        "host": host,
        "required_categories": list(CATEGORIES),
        "observed_inventory": [
            {"id": "host-platform", "kind": "host", "source": "stdlib", "status": "observed"},
            {"id": "host-capacity", "kind": "cpu-and-ram", "source": "stdlib", "status": "observed"},
        ],
        "qualification": [
            {
                "category": category,
                "status": "PHYSICAL_NOT_TESTED",
                "reason": "No separate signed native evidence file was supplied.",
                "native_claim": False,
            }
            for category in CATEGORIES
        ],
        "native_evidence": {
            "supplied": False,
            "accepted": False,
            "signed": False,
            "policy": "Only a separate signed native evidence workflow may change qualification status.",
        },
        "safety": {
            "read_only": True,
            "media_written": False,
            "firmware_changed": False,
            "reboot_requested": False,
            "physical_device_claim": False,
        },
    }


def _validate_report(report: dict[str, Any]) -> None:
    if report["schema"] != SCHEMA:
        raise ValueError("unexpected report schema")
    if report["required_categories"] != list(CATEGORIES):
        raise ValueError("required category list is incomplete or reordered")
    if len(report["qualification"]) != len(CATEGORIES):
        raise ValueError("qualification list is incomplete")
    if any(item["status"] not in QUALIFICATION_STATUSES for item in report["qualification"]):
        raise ValueError("qualification contains an unsafe status")
    if report["native_evidence"]["accepted"]:
        raise ValueError("preflight cannot accept native evidence")


def run(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=Path("audit-artifacts/physical-qualification-preflight"))
    parser.add_argument("--json-only", action="store_true", help="emit only the JSON report on stdout")
    args = parser.parse_args(argv)
    if not is_safe_output_path(args.output_dir):
        parser.error("output path resembles a physical or raw device")
    try:
        output_dir = args.output_dir.resolve()
        output_dir.mkdir(parents=True, exist_ok=True)
        if not output_dir.is_dir():
            raise OSError("output path is not a directory")
        report = build_report()
        _validate_report(report)
        encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
        (output_dir / REPORT_NAME).write_text(encoded, encoding="utf-8", newline="\n")
    except (OSError, ValueError) as exc:
        parser.error(str(exc))
    if not args.json_only:
        print("Physical qualification preflight complete: native qualification remains PHYSICAL_NOT_TESTED.")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(run())
