"""Verify an independent organization's signed reproducible-build attestation."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "wovenhat.independent-build-attestation/v1"
SIGNATURE_SCHEMA = "wovenhat.sdk-provenance-signature/v1"
PROHIBITED_ORGANIZATIONS = {
    "anthony banda",
    "copilot",
    "claude",
    "codex",
    "wovenhat",
    "wovenhat os",
    "woventhat os",
    "rango-ablin",
}


class AttestationError(ValueError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path, label: str) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AttestationError(f"{label} is unreadable: {error}") from error
    if not isinstance(value, dict):
        raise AttestationError(f"{label} must be a JSON object")
    return value


def require_string(document: dict, name: str, *, minimum: int = 1) -> str:
    value = document.get(name)
    if not isinstance(value, str) or len(value.strip()) < minimum:
        raise AttestationError(f"{name} must be a string of at least {minimum} characters")
    return value.strip()


def normalize_organization(value: str) -> str:
    return " ".join(value.casefold().replace("_", " ").replace("-", " ").split())


def verify_signature(signature: dict, attestation: Path, trusted_key: str) -> None:
    if signature.get("schema") != SIGNATURE_SCHEMA:
        raise AttestationError("signature schema is not the supported SDK signature envelope")
    if signature.get("algorithm") != "Ed25519":
        raise AttestationError("attestation signature must use Ed25519")
    if signature.get("public_key") != trusted_key:
        raise AttestationError("attestation signing key is not the pinned independent key")
    environment = dict(
        os.environ,
        WOVENHAT_SDK_TRUSTED_PUBLIC_KEY_HEX=trusted_key,
    )
    verifier = ROOT / "scripts" / "sign-sdk-provenance.py"
    result = subprocess.run(
        [sys.executable, str(verifier), "verify", str(signature["signature_path"]), str(attestation)],
        cwd=ROOT,
        env=environment,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise AttestationError(result.stderr.strip() or "attestation signature verification failed")


def verify(attestation_path: Path, local_report_path: Path, signature_path: Path, trusted_key: str) -> dict:
    if len(trusted_key) != 64:
        raise AttestationError("trusted attestor public key must contain 64 hexadecimal digits")
    try:
        bytes.fromhex(trusted_key)
    except ValueError as error:
        raise AttestationError("trusted attestor public key is not hexadecimal") from error

    attestation = load_json(attestation_path, "attestation")
    local = load_json(local_report_path, "local clean-room report")
    signature = load_json(signature_path, "signature")
    signature = dict(signature)
    signature["signature_path"] = signature_path

    if attestation.get("schema") != SCHEMA:
        raise AttestationError(f"attestation schema must be {SCHEMA}")
    organization = require_string(attestation, "organization", minimum=3)
    if normalize_organization(organization) in PROHIBITED_ORGANIZATIONS:
        raise AttestationError("attestor must not be WovenHat, its owner, or its repository agents")
    require_string(attestation, "organization_legal_name", minimum=3)
    require_string(attestation, "organization_contact", minimum=6)
    require_string(attestation, "attestor_name", minimum=3)
    require_string(attestation, "build_host", minimum=8)
    require_string(attestation, "build_location", minimum=4)
    require_string(attestation, "attested_at_utc", minimum=20)

    if attestation.get("independent") is not True:
        raise AttestationError("attestation must explicitly declare independent=true")
    if attestation.get("source_obtained_from_wovenhat_checkout") is not False:
        raise AttestationError("attestor must confirm it did not build from WovenHat's local checkout")
    if attestation.get("repository_accessed_during_build") is not False:
        raise AttestationError("attestor must confirm the repository was unavailable during the build")

    source_commit = require_string(attestation, "source_commit", minimum=40)
    expected_commit = local.get("source_commit")
    if source_commit != expected_commit:
        raise AttestationError(
            f"source commit mismatch: independent={source_commit} local={expected_commit}"
        )
    archive_hash = require_string(attestation, "archive_sha256", minimum=64)
    if archive_hash != local.get("archive_sha256"):
        raise AttestationError("archive hash does not match the local exact-commit source archive")
    if attestation.get("rustc") != local.get("rustc"):
        raise AttestationError("Rust compiler/toolchain identity does not match the local build")
    if attestation.get("command") != local.get("command"):
        raise AttestationError("build command does not match the accepted locked/offline command")
    if attestation.get("offline_locked") is not True:
        raise AttestationError("independent build must report offline_locked=true")
    if attestation.get("returncode") != 0:
        raise AttestationError("independent build did not pass")
    artifacts = attestation.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        raise AttestationError("artifacts must contain at least one independently hashed artifact")
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            raise AttestationError("each artifact record must be an object")
        require_string(artifact, "path", minimum=1)
        digest = require_string(artifact, "sha256", minimum=64)
        if len(digest) != 64:
            raise AttestationError("artifact sha256 must contain 64 hexadecimal digits")
        try:
            bytes.fromhex(digest)
        except ValueError as error:
            raise AttestationError("artifact sha256 is not hexadecimal") from error

    expected_payload = signature.get("payload")
    if expected_payload and Path(expected_payload).name != attestation_path.name:
        raise AttestationError("signature payload does not identify the supplied attestation")
    if signature.get("payload_sha256") != f"sha256:{sha256(attestation_path)}":
        raise AttestationError("signature payload hash does not match the attestation")
    verify_signature(signature, attestation_path, trusted_key)
    return {
        "schema": "wovenhat.independent-build-verification/v1",
        "valid": True,
        "organization": organization,
        "attestor": attestation["attestor_name"],
        "source_commit": source_commit,
        "archive_sha256": archive_hash,
        "trusted_public_key": trusted_key,
        "artifact_count": len(artifacts),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--attestation", type=Path, required=True)
    parser.add_argument("--signature", type=Path, required=True)
    parser.add_argument("--local-report", type=Path, required=True)
    parser.add_argument("--trusted-public-key", help="64-hex external attestor public key")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    trusted_key = args.trusted_public_key or os.environ.get(
        "WOVENHAT_INDEPENDENT_ATTESTOR_PUBLIC_KEY_HEX"
    )
    if not trusted_key:
        raise SystemExit(
            "WOVENHAT_INDEPENDENT_ATTESTOR_PUBLIC_KEY_HEX or "
            "--trusted-public-key is required"
        )
    try:
        result = verify(
            args.attestation.resolve(),
            args.local_report.resolve(),
            args.signature.resolve(),
            trusted_key.lower(),
        )
    except AttestationError as error:
        print(json.dumps({"valid": False, "error": str(error)}, sort_keys=True))
        return 2
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
