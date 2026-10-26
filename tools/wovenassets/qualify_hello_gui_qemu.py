#!/usr/bin/env python3
"""Fail-closed end-to-end QEMU hello-gui visual qualification.

Requires a preapproved baseline and a real QEMU screenshot; never approves its
own output. The existing source QEMU harness handles guest boot and screenshot.
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path
from capture_hello_gui import capture
from qualify_guest_frame import qualify


def run(root, manifest, baseline_id, evidence_dir, qemu=None, firmware=None,
        timeout=90, capturer=capture, qualifier=qualify):
    evidence = Path(evidence_dir).resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    screenshot = evidence / "hello-gui.png"
    screenshot.unlink(missing_ok=True)
    report_path = evidence / "visual-qualification.json"
    report_path.unlink(missing_ok=True)
    captured = capturer(root, screenshot, qemu, firmware, timeout)
    result = qualifier(manifest, baseline_id, screenshot)
    if not isinstance(result, dict) or type(result.get("pass")) is not bool:
        raise ValueError("qualifier did not return an explicit boolean PASS/FAIL")
    report = {"pass": result["pass"], "capture": captured, "comparison": result}
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--baseline-manifest", type=Path, required=True)
    parser.add_argument("--baseline-id", default="hello-gui")
    parser.add_argument("--evidence-dir", type=Path, required=True)
    parser.add_argument("--qemu")
    parser.add_argument("--firmware")
    parser.add_argument("--timeout", type=int, default=90)
    args = parser.parse_args()
    try:
        outcome = run(args.root, args.baseline_manifest, args.baseline_id,
                      args.evidence_dir, args.qemu, args.firmware, args.timeout)
        print(json.dumps(outcome, indent=2))
        return 0 if outcome["pass"] else 1
    except (OSError, ValueError, RuntimeError, KeyError, subprocess.TimeoutExpired) as error:
        print(json.dumps({"pass": False, "error": str(error)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
