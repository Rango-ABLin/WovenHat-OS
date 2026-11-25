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
    parser.add_argument("--production", action="store_true",
                        help="require the separately managed production key variables")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    environment = os.environ.copy()
    if args.production:
        seed = environment.get("WOVENHAT_SDK_PRODUCTION_SIGNING_SEED_HEX")
        signer = environment.get("WOVENHAT_SDK_PRODUCTION_EXTERNAL_SIGNER")
        public = environment.get("WOVENHAT_SDK_PRODUCTION_TRUSTED_PUBLIC_KEY_HEX")
        if not public:
            raise SystemExit(
                "--production requires WOVENHAT_SDK_PRODUCTION_TRUSTED_PUBLIC_KEY_HEX"
            )
        if bool(seed) == bool(signer):
            raise SystemExit(
                "--production requires exactly one of "
                "WOVENHAT_SDK_PRODUCTION_SIGNING_SEED_HEX or "
                "WOVENHAT_SDK_PRODUCTION_EXTERNAL_SIGNER"
            )
        if seed:
            environment["WOVENHAT_SDK_SIGNING_SEED_HEX"] = seed
        if signer:
            environment["WOVENHAT_SDK_EXTERNAL_SIGNER"] = signer
        environment["WOVENHAT_SDK_TRUSTED_PUBLIC_KEY_HEX"] = public
    command = [
        "cargo", "run", "--quiet", "--locked", "--offline",
        "--manifest-path", str(root / "tools" / "sdk-provenance-signer" / "Cargo.toml"),
        "--", args.command, str(args.first), str(args.second),
    ]
    return subprocess.run(command, cwd=root, env=environment).returncode


if __name__ == "__main__":
    raise SystemExit(main())
