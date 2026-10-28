#!/usr/bin/env python3
"""Validate an immutable WovenInstaller plan and emit canonical JSON.

This is a planning boundary only.  Input is a caller-supplied inventory
snapshot and explicit settings; no device path is opened and no installer,
partitioner, formatter, or bootloader is invoked.
"""

from __future__ import annotations

import argparse
import ipaddress
import json
import re
from pathlib import Path
from typing import Any


MAX_INT = (1 << 63) - 1
SECTOR = 512
RESERVED_BYTES = 1024 * 1024
MIN_PARTITION = 16 * 1024 * 1024
MAX_PARTITIONS = 8
FILESYSTEMS = ("ext4", "fat32", "wovenfs")
KEYBOARDS = ("de", "es", "fr", "gb", "us")
TIMEZONES = ("Africa/Blantyre", "America/New_York", "Asia/Tokyo", "Europe/London", "UTC")
USER_RE = re.compile(r"^[a-z_][a-z0-9_-]{0,31}$")
IDENTITY_RE = re.compile(r"^[^\s]{1,128}$")


class PlanError(ValueError):
    """A deterministic, user-actionable plan validation failure."""


def _require_map(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise PlanError(f"{label} must be an object")
    return value


def _positive_int(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0 or value > MAX_INT:
        raise PlanError(f"{label} must be a positive 64-bit integer")
    return value


def _identity(value: Any, label: str) -> dict[str, str]:
    source = _require_map(value, label)
    normalized: dict[str, str] = {}
    for field in ("path", "serial"):
        item = source.get(field)
        if not isinstance(item, str) or not IDENTITY_RE.fullmatch(item):
            raise PlanError(f"{label}.{field} must be a non-empty identity string")
        normalized[field] = item
    return normalized


def _disk_matches(target: dict[str, str], disk: dict[str, Any]) -> bool:
    return target["path"] == disk.get("path") and target["serial"] == disk.get("serial")


def _validate_target(payload: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    target = _identity(payload.get("target_disk"), "target_disk")
    inventory = payload.get("inventory")
    if not isinstance(inventory, list) or not inventory:
        raise PlanError("inventory must be a non-empty array")
    matches = [disk for disk in inventory if isinstance(disk, dict) and _disk_matches(target, disk)]
    if len(matches) != 1:
        raise PlanError("target identity is missing or ambiguous")
    disk = matches[0]
    capacity = _positive_int(disk.get("size_bytes"), "target disk size_bytes")
    if disk.get("system") is True or disk.get("boot") is True:
        raise PlanError("target disk is protected: system-or-boot")
    if disk.get("removable") is True or disk.get("read_only") is True or disk.get("protected") is True:
        raise PlanError("target disk is protected: removable-readonly-or-explicit")
    labels = " ".join(str(disk.get(key, "")) for key in ("model", "vendor", "media", "bus")).lower()
    if any(word in labels for word in ("backup", "external", "usb")):
        raise PlanError("target disk is protected: backup-external-or-usb")
    if capacity <= RESERVED_BYTES:
        raise PlanError("target disk has insufficient capacity")
    normalized = {"path": target["path"], "serial": target["serial"], "size_bytes": capacity}
    return normalized, disk


def _validate_partitions(payload: dict[str, Any], capacity: int) -> list[dict[str, Any]]:
    partitions = payload.get("partitions")
    if not isinstance(partitions, list) or not 1 <= len(partitions) <= MAX_PARTITIONS:
        raise PlanError(f"partitions must contain 1..{MAX_PARTITIONS} entries")
    result = []
    names: set[str] = set()
    total = 0
    for index, raw in enumerate(partitions):
        item = _require_map(raw, f"partitions[{index}]")
        name = item.get("name")
        if not isinstance(name, str) or not re.fullmatch(r"[a-z][a-z0-9_-]{0,31}", name) or name in names:
            raise PlanError(f"partitions[{index}].name is invalid or duplicated")
        size = _positive_int(item.get("size_bytes"), f"partitions[{index}].size_bytes")
        if size < MIN_PARTITION or size % SECTOR:
            raise PlanError(f"partitions[{index}].size_bytes must be sector-aligned and at least 16 MiB")
        names.add(name)
        total += size
        if total > MAX_INT:
            raise PlanError("partition size total overflows 64-bit capacity")
        result.append({"name": name, "size_bytes": size})
    if total > capacity - RESERVED_BYTES:
        raise PlanError("partition sizes exceed target capacity")
    return result


def _validate_settings(payload: dict[str, Any]) -> dict[str, Any]:
    settings = _require_map(payload.get("settings"), "settings")
    user = _require_map(settings.get("user"), "settings.user")
    username = user.get("username")
    if not isinstance(username, str) or not USER_RE.fullmatch(username):
        raise PlanError("settings.user.username is invalid")
    timezone = settings.get("timezone")
    if timezone not in TIMEZONES:
        raise PlanError("settings.timezone is unsupported")
    keyboard = settings.get("keyboard")
    if keyboard not in KEYBOARDS:
        raise PlanError("settings.keyboard is unsupported")
    network = _require_map(settings.get("network"), "settings.network")
    mode = network.get("mode")
    if mode not in ("disabled", "dhcp", "static"):
        raise PlanError("settings.network.mode is invalid")
    normalized_network: dict[str, Any] = {"mode": mode}
    if mode == "static":
        for field in ("address", "gateway"):
            try:
                normalized_network[field] = str(ipaddress.ip_interface(network[field])) if field == "address" else str(ipaddress.ip_address(network[field]))
            except (KeyError, ValueError, TypeError) as exc:
                raise PlanError(f"settings.network.{field} is invalid") from exc
        dns = network.get("dns")
        if not isinstance(dns, list) or not 1 <= len(dns) <= 3:
            raise PlanError("settings.network.dns must contain 1..3 addresses")
        try:
            normalized_network["dns"] = [str(ipaddress.ip_address(item)) for item in dns]
        except (ValueError, TypeError) as exc:
            raise PlanError("settings.network.dns contains an invalid address") from exc
    elif set(network) != {"mode"}:
        raise PlanError("settings.network fields are not allowed for this mode")
    return {
        "user": {"username": username},
        "timezone": timezone,
        "keyboard": keyboard,
        "network": normalized_network,
    }


def build_plan(payload: Any) -> dict[str, Any]:
    """Return a normalized plan; raise PlanError without side effects."""
    source = _require_map(payload, "plan")
    target, disk = _validate_target(source)
    filesystem = source.get("filesystem")
    if filesystem not in FILESYSTEMS:
        raise PlanError("filesystem must be one of ext4, fat32, wovenfs")
    partitions = _validate_partitions(source, target["size_bytes"])
    settings = _validate_settings(source)
    return {
        "schema": "woveninstaller-plan-v1",
        "operations": {"disk_io": "none", "format": "not-requested", "partition": "not-requested", "installation": "not-implemented"},
        "target_disk": target,
        "partition_table": "gpt",
        "filesystem": filesystem,
        "partitions": partitions,
        "settings": settings,
        "summary": {"partition_count": len(partitions), "partition_bytes": sum(item["size_bytes"] for item in partitions), "target_bytes": target["size_bytes"], "target_model": str(disk.get("model", ""))},
    }


def render(payload: Any) -> str:
    result = {"valid": True, "plan": build_plan(payload)}
    return json.dumps(result, indent=2, sort_keys=True, separators=(",", ": ")) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    args = parser.parse_args()
    try:
        payload = json.loads(args.input.read_text(encoding="utf-8"))
        print(render(payload), end="")
    except (OSError, json.JSONDecodeError, PlanError) as exc:
        print(json.dumps({"valid": False, "error": str(exc)}, sort_keys=True))
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
