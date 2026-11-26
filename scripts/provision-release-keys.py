"""Run the witnessed non-exportable PKCS#11 release-key ceremony."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from datetime import datetime, timezone


PURPOSES = ("boot", "app", "sdk")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-directory",
        type=Path,
        required=True,
        help="directory for public-only custody evidence; must not contain private material",
    )
    parser.add_argument("--token-label", required=True)
    args = parser.parse_args()
    output = args.output_directory.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if any(output.iterdir()):
        raise SystemExit("output directory must be empty")

    root = Path(__file__).resolve().parents[1]
    signer = root / "scripts" / "hardware-release-signer.py"
    records = []
    for purpose in PURPOSES:
        record_path = output / f"{purpose}-public-key.json"
        subprocess.run(
            [sys.executable, str(signer), "provision", purpose, str(record_path)],
            cwd=root,
            env=dict(os.environ),
            check=True,
        )
        record = json.loads(record_path.read_text(encoding="utf-8"))
        records.append(record)
    manifest = {
        "schema": "wovenhat.release-key-custody/v1",
        "created_utc": datetime.now(timezone.utc).isoformat(),
        "token_label": args.token_label,
        "algorithm": "Ed25519",
        "private_material_exported": False,
        "keys": records,
    }
    (output / "custody-manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
    print(f"Created public-only custody evidence in {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
