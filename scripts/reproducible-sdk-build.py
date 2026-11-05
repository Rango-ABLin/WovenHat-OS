"""Build the tracked SDK from an isolated git archive.

This is an independent-workspace reproducibility check, not an independent
security review or a release signature. It deliberately has no access to the
working tree after the archive is created and records the exact command and
toolchain result.
"""

import argparse
import hashlib
import json
import os
import subprocess
import tempfile
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=Path("target/clean-room-build.json"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    archive = subprocess.run(
        ["git", "archive", "--format=tar", "HEAD"],
        cwd=root, check=False, capture_output=True,
    )
    if archive.returncode != 0:
        print(archive.stderr.decode(errors="replace"), end="")
        return archive.returncode
    archive_hash = hashlib.sha256(archive.stdout).hexdigest()
    with tempfile.TemporaryDirectory(prefix="wovenhat-clean-room-") as directory:
        isolated = Path(directory)
        unpack = subprocess.run(["tar", "-xf", "-", "-C", isolated], input=archive.stdout, check=False)
        if unpack.returncode != 0:
            return unpack.returncode
        env = dict(os.environ, CARGO_NET_OFFLINE="true")
        command = ["cargo", "test", "-p", "libwoven", "--lib", "--locked", "--offline"]
        result = subprocess.run(command, cwd=isolated, env=env, check=False, text=True)
        report = {
            "schema": "wovenhat.clean-room-build/v1",
            "archive_sha256": archive_hash,
            "command": command,
            "offline_locked": True,
            "returncode": result.returncode,
            "rustc": subprocess.check_output(["rustc", "-Vv"], text=True).strip(),
        }
    output = root / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    print(output)
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
