"""Small host checks for release-boundary tools and fixed protocol codecs."""

import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class ReleaseBoundaryTests(unittest.TestCase):
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
