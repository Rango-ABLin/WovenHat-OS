#!/usr/bin/env python3
"""Read-only disk inventory and fail-closed safety classification.

This module deliberately consumes command output supplied by the caller.  It
does not execute platform commands, open device paths, mount filesystems, or
change any disk state.  The CLI is therefore safe to run against fixtures and
captured output in CI as well as against output collected by a separate,
reviewed host integration.
"""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Iterable


_SIZE_RE = re.compile(r"^\s*(\d+(?:\.\d+)?)\s*([kmgtpe]?i?b)?\s*$", re.I)
_UNSET = {"", "-", "none", "null", "unknown", "n/a", "not available"}


def _text(value: Any) -> str:
    if value is None:
        return ""
    return str(value).strip()


def _optional_text(value: Any) -> str | None:
    result = _text(value)
    return None if result.lower() in _UNSET else result


def parse_size(value: Any) -> int | None:
    """Convert bytes or a conservative human-size string to bytes."""
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return int(value) if value > 0 and int(value) == value else None
    match = _SIZE_RE.match(_text(value))
    if not match:
        return None
    amount = float(match.group(1))
    unit = (match.group(2) or "b").lower()
    powers = {"b": 0, "kb": 1, "kib": 1, "mb": 2, "mib": 2,
              "gb": 3, "gib": 3, "tb": 4, "tib": 4,
              "pb": 5, "pib": 5, "eb": 6, "eib": 6}
    if unit not in powers:
        return None
    result = int(amount * (1024 ** powers[unit]))
    return result if result > 0 else None


@dataclass(frozen=True)
class Disk:
    """Normalized, read-only disk identity and safety metadata."""

    path: str | None
    number: int | None
    serial: str | None
    size_bytes: int | None
    model: str | None
    vendor: str | None
    bus: str | None
    media: str | None
    system: bool
    boot: bool
    removable: bool
    read_only: bool | None
    source: str


def _flag(value: Any) -> bool:
    if isinstance(value, bool):
        return value
    return _text(value).lower() in {"1", "true", "yes", "on", "removable"}


def _number(value: Any) -> int | None:
    try:
        number = int(value)
    except (TypeError, ValueError):
        return None
    return number if number >= 0 else None


def _normalized(raw: dict[str, Any], source: str) -> Disk:
    return Disk(
        path=_optional_text(raw.get("path", raw.get("Path", raw.get("device")))),
        number=_number(raw.get("number", raw.get("Number"))),
        serial=_optional_text(raw.get("serial", raw.get("SerialNumber"))),
        size_bytes=parse_size(raw.get("size_bytes", raw.get("Size", raw.get("TotalSize", raw.get("size"))))),
        model=_optional_text(raw.get("model", raw.get("Model", raw.get("FriendlyName")))),
        vendor=_optional_text(raw.get("vendor", raw.get("Vendor"))),
        bus=_optional_text(raw.get("bus", raw.get("BusType", raw.get("tran")))),
        media=_optional_text(raw.get("media", raw.get("MediaType"))),
        system=_flag(raw.get("system", raw.get("IsSystem"))),
        boot=_flag(raw.get("boot", raw.get("IsBoot"))),
        removable=_flag(raw.get("removable", raw.get("IsRemovable", raw.get("rm")))),
        read_only=(None if raw.get("read_only", raw.get("IsReadOnly")) is None
                   else _flag(raw.get("read_only", raw.get("IsReadOnly")))),
        source=source,
    )


def _records(payload: Any) -> list[dict[str, Any]]:
    if isinstance(payload, list) and all(isinstance(item, dict) for item in payload):
        return payload
    if isinstance(payload, dict):
        for key in ("disks", "Disks", "blockdevices", "blockdevices"):
            value = payload.get(key)
            if isinstance(value, list) and all(isinstance(item, dict) for item in value):
                return value
        if "blockdevices" in payload and isinstance(payload["blockdevices"], list):
            return [item for item in payload["blockdevices"] if isinstance(item, dict)]
    raise ValueError("fixture must contain a JSON array of disk records")


def parse_fixture(text: str, fmt: str = "normalized") -> list[Disk]:
    """Parse supplied JSON output; no host discovery is performed."""
    try:
        payload = json.loads(text)
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON fixture: {exc}") from exc
    records = _records(payload)
    if fmt not in {"normalized", "powershell", "lsblk", "auto"}:
        raise ValueError(f"unsupported fixture format: {fmt}")
    source = "normalized" if fmt in {"normalized", "auto"} else fmt
    disks = [_normalized(record, source) for record in records]
    if fmt == "lsblk":
        disks = [disk for disk, record in zip(disks, records)
                 if _text(record.get("type", "disk")) in {"", "disk"}]
    if not disks:
        raise ValueError("fixture contains no disk records")
    return disks


def _identity_errors(disk: Disk) -> list[str]:
    errors = []
    if not disk.path and disk.number is None:
        errors.append("missing path-or-number")
    if not disk.serial:
        errors.append("missing-serial")
    if disk.size_bytes is None:
        errors.append("missing-or-invalid-capacity")
    return errors


def classify(disks: Iterable[Disk]) -> dict[str, Any]:
    """Return a conservative report suitable for display, never for writes."""
    items = list(disks)
    serials = [disk.serial.casefold() for disk in items if disk.serial]
    duplicate_serials = sorted({serial for serial in serials if serials.count(serial) > 1})
    system_candidates = [index for index, disk in enumerate(items) if disk.system or disk.boot]
    protected = []
    protected_details = []
    for index, disk in enumerate(items):
        text = " ".join(filter(None, (disk.model, disk.vendor, disk.media, disk.bus))).casefold()
        reasons = []
        if disk.removable:
            reasons.append("removable")
        reasons.extend(word for word in ("backup", "external", "usb") if word in text)
        if reasons:
            protected.append(index)
            protected_details.append({"index": index, "reasons": sorted(set(reasons))})
    identities = []
    for index, disk in enumerate(items):
        errors = _identity_errors(disk)
        if disk.serial and disk.serial.casefold() in duplicate_serials:
            errors.append("duplicate-serial")
        identities.append({"index": index, "complete": not errors, "errors": errors})
    identity_ok = all(item["complete"] for item in identities) and not duplicate_serials
    system_ok = len(system_candidates) == 1
    return {
        "read_only": True,
        "disk_count": len(items),
        "system_disk": (system_candidates[0] if system_ok else None),
        "system_candidates": system_candidates,
        "protected_candidates": protected,
        "protected_candidate_details": protected_details,
        "identity": identities,
        "duplicate_serials": duplicate_serials,
        "safe_target_selection": bool(identity_ok and system_ok),
        "blocking_reasons": (["system-disk-ambiguous-or-unmarked"] if not system_ok else [])
        + ([] if identity_ok else ["disk-identity-or-capacity-ambiguous"]),
        "disks": [asdict(disk) for disk in items],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path, help="JSON fixture/captured output")
    parser.add_argument("--format", choices=("normalized", "powershell", "lsblk", "auto"), default="auto")
    args = parser.parse_args()
    try:
        report = classify(parse_fixture(args.input.read_text(encoding="utf-8-sig"), args.format))
    except (OSError, ValueError) as exc:
        parser.error(str(exc))
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["safe_target_selection"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
