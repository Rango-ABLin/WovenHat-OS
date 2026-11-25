"""Create an unsigned independent-organization build attestation."""

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path


SCHEMA = "wovenhat.independent-build-attestation/v1"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--organization", required=True)
    parser.add_argument("--organization-legal-name", required=True)
    parser.add_argument("--organization-contact", required=True)
    parser.add_argument("--attestor-name", required=True)
    parser.add_argument("--build-host", required=True)
    parser.add_argument("--build-location", required=True)
    parser.add_argument("--artifact", action="append", default=[])
    args = parser.parse_args()
    report = json.loads(args.build_report.read_text(encoding="utf-8"))
    if (
        report.get("schema") != "wovenhat.clean-room-build/v1"
        or report.get("returncode") != 0
        or report.get("offline_locked") is not True
    ):
        raise SystemExit("build report is not a passing locked/offline clean-room report")
    artifacts = []
    for value in args.artifact:
        path, separator, digest = value.partition("=")
        if not separator or len(digest) != 64:
            raise SystemExit("--artifact must be PATH=64_HEX_SHA256")
        try:
            bytes.fromhex(digest)
        except ValueError as error:
            raise SystemExit("--artifact SHA-256 is not hexadecimal") from error
        artifacts.append({"path": path, "sha256": digest.lower()})
    if not artifacts:
        artifacts.append(
            {
                "path": "isolated-libwoven-test-result",
                "sha256": report["archive_sha256"],
            }
        )
    document = {
        "schema": SCHEMA,
        "organization": args.organization,
        "organization_legal_name": args.organization_legal_name,
        "organization_contact": args.organization_contact,
        "attestor_name": args.attestor_name,
        "build_host": args.build_host,
        "build_location": args.build_location,
        "attested_at_utc": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "independent": True,
        "source_obtained_from_wovenhat_checkout": False,
        "repository_accessed_during_build": False,
        "source_commit": report["source_commit"],
        "archive_sha256": report["archive_sha256"],
        "command": report["command"],
        "offline_locked": report["offline_locked"],
        "returncode": report["returncode"],
        "rustc": report["rustc"],
        "artifacts": artifacts,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
