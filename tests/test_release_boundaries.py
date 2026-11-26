"""Small host checks for release-boundary tools and fixed protocol codecs."""

import json
import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ATTESTATION_SCRIPT = ROOT / "scripts" / "verify-independent-build-attestation.py"
ATTESTATION_SPEC = importlib.util.spec_from_file_location(
    "verify_independent_build_attestation", ATTESTATION_SCRIPT
)
ATTESTATION = importlib.util.module_from_spec(ATTESTATION_SPEC)
assert ATTESTATION_SPEC.loader is not None
ATTESTATION_SPEC.loader.exec_module(ATTESTATION)

TEST_SEED = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"
TEST_PUBLIC_KEY = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"


class ReleaseBoundaryTests(unittest.TestCase):
    def test_clean_room_sdk_rebuild_is_locked_offline_and_isolated(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "clean-room-build.json"
            result = subprocess.run(
                [
                    sys.executable,
                    "scripts/reproducible-sdk-build.py",
                    "--output",
                    str(output),
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            report = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(report["schema"], "wovenhat.clean-room-build/v1")
            self.assertTrue(report["archive_sha256"])
            self.assertTrue(report["offline_locked"])
            self.assertEqual(report["returncode"], 0)
            self.assertTrue(report["command"])
            self.assertIn("--locked", report["command"])
            self.assertIn("--offline", report["command"])
            self.assertIn("rustc ", report["rustc"])

    def test_provenance_tool_is_locked_and_offline(self):
        result = subprocess.run(
            [sys.executable, "scripts/verify-clean-room-sdk.py", "--output", "target/test-sdk-provenance.json"],
            cwd=ROOT, capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        document = json.loads((ROOT / "target/test-sdk-provenance.json").read_text(encoding="utf-8"))
        self.assertTrue(document["offline_locked_metadata"])
        self.assertTrue(document["source_sha256"])
        self.assertIn("libwoven@0.1.0", document["packages"])
        self.assertEqual(document["sbom"]["bomFormat"], "CycloneDX")
        self.assertTrue(any(component["name"] == "libwoven" for component in document["sbom"]["components"]))

    def test_independent_build_attestation_rejects_local_identity_and_tamper(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            local = {
                "schema": "wovenhat.clean-room-build/v1",
                "source_commit": "a" * 40,
                "archive_sha256": "b" * 64,
                "command": ["cargo", "test", "-p", "libwoven", "--lib", "--locked", "--offline"],
                "offline_locked": True,
                "returncode": 0,
                "rustc": "rustc 1.99.0-nightly (test)",
            }
            attestation = {
                "schema": ATTESTATION.SCHEMA,
                "organization": "Independent Build Laboratory",
                "organization_legal_name": "Independent Build Laboratory Ltd.",
                "organization_contact": "attestation@example.test",
                "attestor_name": "Named Operator",
                "build_host": "organization-builder-01",
                "build_location": "independent controlled environment",
                "attested_at_utc": "2026-11-25T15:30:00Z",
                "independent": True,
                "source_obtained_from_wovenhat_checkout": False,
                "repository_accessed_during_build": False,
                "source_commit": local["source_commit"],
                "archive_sha256": local["archive_sha256"],
                "command": local["command"],
                "offline_locked": True,
                "returncode": 0,
                "rustc": local["rustc"],
                "artifacts": [{"path": "libwoven-test", "sha256": "c" * 64}],
            }
            local_path = root / "local.json"
            attestation_path = root / "attestation.json"
            signature_path = root / "attestation.signature.json"
            local_path.write_text(json.dumps(local), encoding="utf-8")
            attestation_path.write_text(json.dumps(attestation), encoding="utf-8")
            environment = dict(
                os.environ,
                WOVENHAT_SDK_SIGNING_SEED_HEX=TEST_SEED,
                WOVENHAT_SDK_TRUSTED_PUBLIC_KEY_HEX=TEST_PUBLIC_KEY,
            )
            subprocess.run(
                [
                    sys.executable,
                    "scripts/sign-sdk-provenance.py",
                    "sign",
                    str(attestation_path),
                    str(signature_path),
                ],
                cwd=ROOT,
                env=environment,
                check=True,
                capture_output=True,
                text=True,
            )
            result = ATTESTATION.verify(
                attestation_path, local_path, signature_path, TEST_PUBLIC_KEY
            )
            self.assertTrue(result["valid"])

            attestation["organization"] = "WovenHat"
            attestation_path.write_text(json.dumps(attestation), encoding="utf-8")
            with self.assertRaisesRegex(ATTESTATION.AttestationError, "must not be WovenHat"):
                ATTESTATION.verify(
                    attestation_path, local_path, signature_path, TEST_PUBLIC_KEY
                )

            attestation["organization"] = "Independent Build Laboratory"
            attestation["attestor_name"] = "Different Unsigned Operator"
            attestation_path.write_text(json.dumps(attestation), encoding="utf-8")
            with self.assertRaisesRegex(ATTESTATION.AttestationError, "payload hash"):
                ATTESTATION.verify(
                    attestation_path, local_path, signature_path, TEST_PUBLIC_KEY
                )


if __name__ == "__main__":
    unittest.main()
