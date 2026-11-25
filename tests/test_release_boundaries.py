"""Small host checks for release-boundary tools and fixed protocol codecs."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


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


if __name__ == "__main__":
    unittest.main()
