"""Linux CI mutation control: remove only the tag comparison in a private checkout.
The unchanged guest assertion MUST then observe stale remap acceptance. Build,
timeout and unrelated guest failures cannot count as a reproduced defect.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

MARKER = "[WOVENWEB-VM] stale queued remap exit=Some(0) resident=Some(1) expected=-11/0 requests=1 completed=1"

def verify_observation(result, status, serial):
    if status != 1 or result.get("passed") is not False or result.get("exit") != 35 or MARKER not in serial:
        raise ValueError("negative control did not reproduce precise stale-remap acceptance")
    for required in ["remap phase=old-fault-queued", "remap phase=ready-file-published", "queued sibling page deltas + duplicate coalescing + exact cleanup: PASSED"]:
        if required not in serial:
            raise ValueError("negative control did not reach the remap prerequisite")

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", required=True)
    parser.add_argument("--firmware", required=True)
    parser.add_argument("--firmware-vars", required=True)
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("run on the Linux CI runner; Windows application policy stays unchanged")
    root = Path(__file__).resolve().parents[1]
    source = root / "kernel/src/userspace.rs"
    original = digest(source)
    out = root / "audit-artifacts" / f"wovenweb-generation-negative-{time.time_ns()}"
    out.mkdir(parents=True)
    scratch = Path(tempfile.mkdtemp(prefix="wovenweb-generation-")) / "checkout"
    subprocess.run(["git", "worktree", "add", "--detach", "--quiet", str(scratch), "HEAD"], cwd=root, check=True)
    # This detached runner checkout is disposable. The primary source is never
    # modified and the OS's cleanup removes the ephemeral checkout after the job.
    target = scratch / "kernel/src/userspace.rs"
    text = target.read_text()
    needle = "            && self.lifetime == other.lifetime\n"
    if text.count(needle) != 1:
        raise RuntimeError("tag comparison changed; review the mutation explicitly")
    target.write_text(text.replace(needle, "", 1))
    (out / "source-mutation.patch").write_bytes(subprocess.check_output(["git", "diff", "--", "kernel/src/userspace.rs"], cwd=scratch))
    command = [sys.executable, str(scratch / "scripts/test-wovenweb-vm-qemu.py"), "--cpus", "1", "--qemu", args.qemu, "--firmware", args.firmware, "--firmware-vars", args.firmware_vars]
    try:
        with (out / "runner.log").open("w") as log:
            status = subprocess.run(command, cwd=scratch, stdout=log, stderr=subprocess.STDOUT).returncode
        artifacts = list((scratch / "audit-artifacts").glob("wovenweb-vm-*"))
        if len(artifacts) != 1:
            raise RuntimeError("ambiguous/missing negative guest evidence")
        evidence = artifacts[0]
        shutil.copytree(evidence, out / "guest")
        result = json.loads((evidence / "result.json").read_text())
        verify_observation(result, status, (evidence / "serial.log").read_text())
        (out / "result.json").write_text(json.dumps({"reproduced": True, "primary_source_unchanged": digest(source) == original, "primary_sha256": original, "mutated_sha256": digest(target), "guest": result}, indent=2))
    finally:
        if digest(source) != original:
            raise RuntimeError("primary checkout was unexpectedly changed")
    print("Negative stale-remap control reproduced:", out)
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
