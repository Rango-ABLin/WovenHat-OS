#!/usr/bin/env python3
"""Capture an actual QEMU hello-gui frame using the existing project harness.

This bridge never creates or approves baselines. A capture passes only after
the underlying QEMU harness succeeds and emits a nonempty PNG.
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path
from PIL import Image


def capture(root, output, qemu=None, firmware=None, timeout=90, runner=subprocess.run):
    root = Path(root).resolve()
    output = Path(output).resolve()
    harness = root / "scripts" / "screenshot-hello-gui-qemu.py"
    if not harness.is_file():
        raise ValueError("existing QEMU screenshot harness not found")
    if not 1 <= timeout <= 600:
        raise ValueError("invalid timeout")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.unlink(missing_ok=True)
    command = [sys.executable, str(harness), "--out", str(output), "--timeout", str(timeout)]
    if qemu:
        command.extend(["--qemu", str(qemu)])
    if firmware:
        command.extend(["--firmware", str(firmware)])
    result = runner(command, cwd=root, capture_output=True, text=True, timeout=timeout + 180)
    if result.returncode:
        raise RuntimeError("QEMU capture failed: " + result.stderr[-1500:])
    if not output.is_file() or output.stat().st_size == 0:
        raise ValueError("QEMU harness returned without a PNG")
    with Image.open(output) as frame:
        frame.verify()
    with Image.open(output) as frame:
        if frame.format != "PNG" or min(frame.size) <= 0:
            raise ValueError("invalid QEMU PNG evidence")
        return {"capture": str(output), "size": list(frame.size), "harness": str(harness)}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--root",type=Path,default=Path(__file__).resolve().parents[2])
    p.add_argument("--output",required=True,type=Path)
    p.add_argument("--qemu")
    p.add_argument("--firmware")
    p.add_argument("--timeout",type=int,default=90)
    args=p.parse_args()
    try:
        print(json.dumps(capture(args.root,args.output,args.qemu,args.firmware,args.timeout),indent=2))
        return 0
    except (OSError,ValueError,RuntimeError,subprocess.TimeoutExpired) as e:
        print(json.dumps({"pass":False,"error":str(e)}),file=sys.stderr)
        return 2
if __name__=="__main__":
    sys.exit(main())
