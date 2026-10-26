#!/usr/bin/env python3
"""Verify explicit QEMU guest captures against *reviewed* baseline metadata.

Exit status: 0 all passed; 1 a visual difference; 2 absent/invalid evidence.
No guest capture or reference is synthesized here.
"""
import argparse
import json
import sys
from pathlib import Path
from qualify_guest_frame import qualify


def run(manifest, captures):
    items = json.loads(Path(captures).read_text(encoding="utf-8"))
    if items.get("schema_version") != 1 or not isinstance(items.get("captures"), list):
        raise ValueError("invalid capture map")
    pairs = items["captures"]
    if not pairs:
        raise ValueError("empty capture map: no guest evidence")
    seen = set()
    results = []
    for entry in pairs:
        if not isinstance(entry, dict) or set(entry) != {"id", "path"}:
            raise ValueError("capture map entry must contain id and path")
        identifier, relative = entry["id"], entry["path"]
        if not isinstance(identifier, str) or not identifier or identifier in seen:
            raise ValueError("invalid or duplicate capture identifier")
        seen.add(identifier)
        if not isinstance(relative, str) or not relative:
            raise ValueError("missing capture path")
        candidate = Path(relative)
        if candidate.is_absolute() or ".." in candidate.parts or candidate.suffix.lower() != ".png":
            raise ValueError("unsafe capture path")
        path = (Path(captures).parent / candidate).resolve()
        if not path.is_relative_to(Path(captures).parent.resolve()):
            raise ValueError("capture escapes evidence directory")
        if not path.is_file():
            raise ValueError("guest screenshot not present: " + identifier)
        results.append(qualify(manifest, identifier, path))
    manifest_data = json.loads(Path(manifest).read_text(encoding="utf-8"))
    expected = {b["id"] for b in manifest_data.get("baselines", [])}
    if seen != expected:
        raise ValueError("capture set differs from reviewed baselines")
    return {"pass": all(item["pass"] for item in results), "captures": results}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("baseline_manifest")
    p.add_argument("capture_map")
    p.add_argument("--report")
    args = p.parse_args()
    try:
        result = run(args.baseline_manifest, args.capture_map)
        if args.report:
            Path(args.report).write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result, indent=2))
        return 0 if result["pass"] else 1
    except (OSError, ValueError, KeyError, TypeError) as exc:
        print(json.dumps({"pass": False, "error": str(exc)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
