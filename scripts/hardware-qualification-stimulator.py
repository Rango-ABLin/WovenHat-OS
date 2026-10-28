#!/usr/bin/env python3
"""Bounded, deterministic virtual-hardware qualification stimulator.

This tool plans and records software/emulated qualification scenarios.  It
does not start a hypervisor, probe hardware, open raw devices, or certify a
native device.  The output is intentionally explicit about that boundary.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
MATRIX_PATH = ROOT / "tests" / "hardware-matrix.json"
ALLOWED_CPUS = (1, 2, 4, 8, 16)
FORBIDDEN_ARGUMENTS = {
    "--device",
    "--physical-device",
    "--raw-disk",
    "--raw-device",
    "--native",
    "--tier1",
}
FORBIDDEN_PATH_MARKERS = (
    "/dev/",
    "\\\\.\\physicaldrive",
    "\\\\.\\scsi",
    "\\\\.\\usb",
)


class QualificationError(ValueError):
    """Raised for an invalid or unsafe qualification request."""


def load_matrix(path: Path = MATRIX_PATH) -> dict[str, Any]:
    """Read the qualification contract without accessing any device path."""
    try:
        matrix = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise QualificationError(f"cannot read hardware matrix: {exc}") from exc
    if not isinstance(matrix, dict) or not isinstance(matrix.get("profiles"), list):
        raise QualificationError("hardware matrix has no valid profiles list")
    return matrix


def _reject_physical_arguments(argv: Iterable[str]) -> None:
    for raw in argv:
        token = raw.lower()
        if token in FORBIDDEN_ARGUMENTS:
            raise QualificationError(f"physical/native option is forbidden: {raw}")
        if any(marker in token for marker in FORBIDDEN_PATH_MARKERS):
            raise QualificationError(f"physical-device path is forbidden: {raw}")


def _seed(value: str) -> int:
    try:
        number = int(value, 0)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("seed must be an integer (decimal or 0x-prefixed)") from exc
    if not 0 <= number <= 0xFFFFFFFFFFFFFFFF:
        raise argparse.ArgumentTypeError("seed must fit in an unsigned 64-bit integer")
    return number


def _positive_int(value: str) -> int:
    try:
        number = int(value, 10)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("value must be a positive integer") from exc
    if number <= 0:
        raise argparse.ArgumentTypeError("value must be a positive integer")
    return number


def _profile_map(matrix: dict[str, Any]) -> dict[str, dict[str, Any]]:
    profiles: dict[str, dict[str, Any]] = {}
    for item in matrix["profiles"]:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str):
            raise QualificationError("hardware matrix contains a malformed profile")
        profiles[item["id"]] = item
    return profiles


def _choose_profiles(requested: list[str] | None, profiles: dict[str, dict[str, Any]]) -> list[str]:
    if not requested or "all" in requested:
        if requested and set(requested) != {"all"}:
            raise QualificationError("--profile all cannot be combined with a profile id")
        return list(profiles)
    unknown = [profile for profile in requested if profile not in profiles]
    if unknown:
        raise QualificationError(f"invalid profile: {', '.join(unknown)}")
    return list(dict.fromkeys(requested))


def _choose_faults(requested: list[str], selected: list[str], profiles: dict[str, dict[str, Any]]) -> list[str]:
    available = {
        fault
        for profile in selected
        for fault in profiles[profile].get("faults", [])
    }
    unknown = [fault for fault in requested if fault not in available]
    if unknown:
        raise QualificationError(f"invalid fault for selected profile(s): {', '.join(unknown)}")
    return list(dict.fromkeys(requested))


def _event_token(seed: int, profile: str, index: int, label: str) -> str:
    material = f"{seed:016x}:{profile}:{index}:{label}".encode("utf-8")
    return hashlib.sha256(material).hexdigest()[:16]


def _events(seed: int, profile: str, faults: list[str]) -> list[dict[str, Any]]:
    labels = ["matrix-validated", "backend-reserved", "input-ready", "fault-boundary", "evidence-captured"]
    events = []
    for index, label in enumerate(labels):
        events.append({
            "sequence": index,
            "event": label,
            "token": _event_token(seed, profile, index, label),
        })
    events[3]["faults"] = faults
    return events


def _safe_output_dir(raw: str) -> Path:
    lowered = raw.lower()
    if any(marker in lowered for marker in FORBIDDEN_PATH_MARKERS):
        raise QualificationError("output directory cannot be a physical-device path")
    path = Path(raw)
    if path.name in {"", ".", ".."}:
        raise QualificationError("output directory must name a directory")
    return path


def _status(profile: dict[str, Any]) -> str:
    backend = profile.get("backend")
    if backend in {"software-fixture", "qemu-null-sink"}:
        return "SOFTWARE_ONLY_PASS"
    return "EMULATED_PASS"


def build_result(matrix: dict[str, Any], seed: int, cpus: int, ram_mib: int,
                 selected: list[str], faults: list[str], output_dir: Path) -> dict[str, Any]:
    profiles = _profile_map(matrix)
    records = []
    for profile_id in selected:
        profile = profiles[profile_id]
        evidence_name = re.sub(r"[^A-Za-z0-9_.-]+", "_", profile_id) + ".json"
        evidence_path = output_dir / "evidence" / evidence_name
        profile_faults = [fault for fault in faults if fault in profile.get("faults", [])]
        record = {
            "profile": profile_id,
            "class": profile.get("class"),
            "backend": profile.get("backend"),
            "seed": seed,
            "cpu_count": cpus,
            "ram_mib": ram_mib,
            "deterministic_event_sequence": _events(seed, profile_id, profile_faults),
            "injected_faults": profile_faults,
            "evidence_file": str(evidence_path),
            "status": _status(profile),
            "physical_status": "PHYSICAL_NOT_TESTED",
            "physical_claim": False,
            "native_claim": False,
            "tier1_claim": False,
        }
        records.append(record)
    return {
        "schema": 1,
        "kind": "wovenhat-hardware-qualification-stimulator-result",
        "seed": seed,
        "cpu_count": cpus,
        "ram_mib": ram_mib,
        "profiles": records,
        "status_values": matrix.get("status_values", []),
        "safety": {
            "physical_device_access": False,
            "raw_disk_access": False,
            "native_hardware_claim": False,
            "tier1_claim": False,
            "qualification_boundary": "bounded emulation/software planning only",
        },
    }


def _write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def run(argv: list[str]) -> int:
    _reject_physical_arguments(argv)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", type=_seed, default=0x51A7E)
    parser.add_argument("--cpus", type=int, choices=ALLOWED_CPUS, default=1)
    parser.add_argument("--ram-mib", type=_positive_int, default=4096)
    parser.add_argument("--profile", action="append", help="profile id, repeatable, or all")
    parser.add_argument("--fault", action="append", default=[], help="fault id, repeatable")
    parser.add_argument("--output-dir", default="audit-artifacts/hardware-qualification-stimulator")
    args = parser.parse_args(argv)

    matrix = load_matrix()
    profiles = _profile_map(matrix)
    allowed_ram = set(matrix.get("ram_mib", []))
    if args.ram_mib not in allowed_ram:
        raise QualificationError(f"invalid RAM size: {args.ram_mib}; allowed values: {sorted(allowed_ram)}")
    selected = _choose_profiles(args.profile, profiles)
    faults = _choose_faults(args.fault, selected, profiles)
    output_dir = _safe_output_dir(args.output_dir)
    result = build_result(matrix, args.seed, args.cpus, args.ram_mib, selected, faults, output_dir)

    manifest = {
        "schema": 1,
        "kind": "wovenhat-hardware-qualification-stimulator-manifest",
        "matrix": str(MATRIX_PATH),
        "seed": args.seed,
        "cpu_count": args.cpus,
        "ram_mib": args.ram_mib,
        "profiles": selected,
        "faults": faults,
        "result_file": str(output_dir / "result.json"),
        "evidence_files": [record["evidence_file"] for record in result["profiles"]],
        "status_values": matrix.get("status_values", []),
        "physical_status": "PHYSICAL_NOT_TESTED",
        "physical_device_access": False,
        "native_hardware_claim": False,
        "tier1_claim": False,
    }
    _write_json(output_dir / "manifest.json", manifest)
    for record in result["profiles"]:
        _write_json(Path(record["evidence_file"]), record)
    _write_json(output_dir / "result.json", result)
    print(json.dumps({"status": "generated", "result": str(output_dir / "result.json")}, sort_keys=True))
    return 0


def main() -> int:
    try:
        return run(sys.argv[1:])
    except (QualificationError, OSError) as exc:
        print(f"hardware-qualification-stimulator: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
