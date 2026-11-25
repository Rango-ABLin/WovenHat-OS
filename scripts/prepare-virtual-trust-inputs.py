#!/usr/bin/env python3
"""Stage deterministic virtual Secure Boot inputs for the trust harness.

The OVMF snakeoil VARS template is already enrolled with a test Secure Boot
key. This script copies that template, its matching CODE image, certificate and
a decrypted private key into a caller-owned directory so
``test-virtual-trust-chain.py`` can run without a manual WSL/efitools ceremony.

It is a disposable virtual-input generator only: no device, firmware variable,
host boot entry or production key is accessed or changed. The generated key is
a well-known distribution test key and must never be used for a release.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys


def _load_harness():
    # The harness filename contains hyphens, so load it by path rather than
    # as a normal import.
    path = Path(__file__).resolve().parent / "test-virtual-trust-chain.py"
    spec = importlib.util.spec_from_file_location("test_virtual_trust_chain", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


_harness = _load_harness()
TrustInputError = _harness.TrustInputError
validate_development_key_material = _harness.validate_development_key_material

SOURCES = {
    "ovmf_code": "/usr/share/OVMF/OVMF_CODE_4M.snakeoil.fd",
    "ovmf_vars": "/usr/share/OVMF/OVMF_VARS_4M.snakeoil.fd",
    "certificate": "/usr/share/ovmf/PkKek-1-snakeoil.pem",
    "encrypted_key": "/usr/share/ovmf/PkKek-1-snakeoil.key",
}
SNAKEOIL_PASSWORD = "snakeoil"
MAX_MANIFEST_BYTES = 65536


def wsl_path(path: Path) -> str:
    """Convert an absolute Windows path to the WSL mount path."""
    text = str(path.resolve())
    if len(text) >= 2 and text[1] == ":":
        return "/mnt/" + text[0].lower() + text[2:].replace("\\", "/")
    return text.replace("\\", "/")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run_wsl(distro: str, script: str) -> str:
    wsl = shutil.which("wsl.exe")
    if wsl is None:
        raise TrustInputError("wsl.exe is required to stage the OVMF snakeoil inputs")
    result = subprocess.run(
        [wsl, "-d", distro, "-e", "sh", "-lc", script],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise TrustInputError(f"WSL staging failed: {result.stderr.strip()}")
    return result.stdout


def stage(distro: str, output: Path) -> dict[str, object]:
    for name in SOURCES:
        source = Path(SOURCES[name]).resolve()
        if output.resolve() == source:
            raise TrustInputError("output directory collides with a source path")
    output.mkdir(parents=True, exist_ok=False)
    target = wsl_path(output)
    script = (
        "set -eu\n"
        + f"test -f {SOURCES['ovmf_code']}\n"
        + f"test -f {SOURCES['ovmf_vars']}\n"
        + f"test -f {SOURCES['certificate']}\n"
        + f"test -f {SOURCES['encrypted_key']}\n"
        + f"cp {SOURCES['ovmf_code']} {target}/ovmf-code.fd\n"
        + f"cp {SOURCES['ovmf_vars']} {target}/ovmf-vars.fd\n"
        + f"cp {SOURCES['certificate']} {target}/dev-cert.pem\n"
        + f"openssl pkey -in {SOURCES['encrypted_key']} -passin pass:{SNAKEOIL_PASSWORD} "
        + f"-out {target}/dev-key.pem\n"
    )
    run_wsl(distro, script)
    names = {
        "ovmf_code": "ovmf-code.fd",
        "ovmf_vars": "ovmf-vars.fd",
        "certificate": "dev-cert.pem",
        "key": "dev-key.pem",
    }
    files = {role: output / name for role, name in names.items()}
    for role, path in files.items():
        if not path.is_file() or path.stat().st_size == 0:
            raise TrustInputError(f"staged {role} is missing or empty")
    return files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new directory for virtual inputs")
    parser.add_argument("--distro", default="Ubuntu", help="WSL distro with OVMF and OpenSSL")
    parser.add_argument(
        "--openssl",
        default=str(Path("C:/Program Files/Git/usr/bin/openssl.exe")),
        help="OpenSSL used to validate the decrypted development key pair",
    )
    args = parser.parse_args()
    if args.output.exists():
        parser.error("--output must be a new directory")
    try:
        files = stage(args.distro, args.output)
        checked = validate_development_key_material(
            files["key"], files["certificate"], Path(args.openssl)
        )
        manifest = {
            "schema": "wovenhat.virtual-trust-inputs/v1",
            "distro": args.distro,
            "sources": SOURCES,
            "files": {
                role: {"path": path.name, "sha256": sha256(path)}
                for role, path in files.items()
            },
            "certificate_sha256": checked["certificate_sha256"],
            "physical_access": False,
            "firmware_mutation": False,
            "production_key": False,
            "release_qualified": False,
        }
        encoded = json.dumps(manifest, indent=2, sort_keys=True) + "\n"
        if len(encoded.encode("utf-8")) > MAX_MANIFEST_BYTES:
            raise TrustInputError("manifest exceeded its bound")
        (args.output / "manifest.json").write_text(encoded, encoding="utf-8")
    except (TrustInputError, OSError) as error:
        print(f"virtual trust input staging failed: {error}", file=sys.stderr)
        return 1
    print(f"Virtual trust inputs staged: {args.output}")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
