"""Produce or verify a detached production signature for SDK provenance."""

import argparse
import os
import subprocess
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("sign", "verify"))
    parser.add_argument("first", type=Path)
    parser.add_argument("second", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    command = [
        "cargo", "run", "--quiet", "--locked", "--offline",
        "--manifest-path", str(root / "tools" / "sdk-provenance-signer" / "Cargo.toml"),
        "--", args.command, str(args.first), str(args.second),
    ]
    return subprocess.run(command, cwd=root, env=os.environ.copy()).returncode


if __name__ == "__main__":
    raise SystemExit(main())
