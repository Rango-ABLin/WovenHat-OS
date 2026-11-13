"""Adversarial input tests for the actual screenshot evidence gate.

These are host tests only and make no claim of booting the WovenHat guest.
"""
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image
from qualify_guest_frame import qualify
from run_guest_visual_gate import run


class HostEvidenceNegativeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.reference = self.root / "reference.png"
        self.capture = self.root / "capture.png"
        Image.new("RGB", (8, 8), (1, 2, 3)).save(self.reference)
        Image.new("RGB", (8, 8), (1, 2, 3)).save(self.capture)
        self.manifest = self.root / "baseline.json"
        self.map = self.root / "captures.json"
        self.base = {
            "id": "desktop", "path": "reference.png",
            "sha256": hashlib.sha256(self.reference.read_bytes()).hexdigest(),
            "reviewed_by": "reviewed", "guest_commit": "fixture-commit",
            "capture_method": "QMP", "size": [8, 8],
            "max_abs": 0, "mean_abs": 0,
        }
        self.write()

    def write(self):
        self.manifest.write_text(json.dumps({
            "schema_version": 1, "kind": "wovenhat-guest-baselines",
            "baselines": [self.base],
        }))
        self.map.write_text(json.dumps({
            "schema_version": 1, "captures": [
                {"id": "desktop", "path": "capture.png"}
            ],
        }))

    def test_matching_evidence(self):
        self.assertTrue(run(self.manifest, self.map)["pass"])

    def test_corrupt_guest_png_rejected(self):
        self.capture.write_bytes(b"this is not a PNG")
        with self.assertRaises(OSError):
            run(self.manifest, self.map)

    def test_unsupported_reference_hash_rejected(self):
        self.base["sha256"] = "not-a-sha256"
        self.write()
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            qualify(self.manifest, "desktop", self.capture)

    def test_malformed_manifest_rejected(self):
        self.manifest.write_text("not JSON")
        with self.assertRaises(json.JSONDecodeError):
            run(self.manifest, self.map)

    def test_relative_escape_rejected(self):
        self.map.write_text(json.dumps({
            "schema_version": 1, "captures": [
                {"id": "desktop", "path": "../escape.png"}
            ],
        }))
        with self.assertRaisesRegex(ValueError, "unsafe"):
            run(self.manifest, self.map)

    def test_unlisted_extra_capture_rejected(self):
        self.map.write_text(json.dumps({
            "schema_version": 1, "captures": [
                {"id": "desktop", "path": "capture.png"},
                {"id": "extra", "path": "capture.png"},
            ],
        }))
        with self.assertRaises(ValueError):
            run(self.manifest, self.map)

    def test_missing_reference_rejected(self):
        self.reference.unlink()
        with self.assertRaisesRegex(ValueError, "missing"):
            qualify(self.manifest, "desktop", self.capture)

    def test_reference_dimensions_mismatch_rejected(self):
        self.base["size"] = [10, 10]
        self.write()
        with self.assertRaisesRegex(ValueError, "dimensions"):
            qualify(self.manifest, "desktop", self.capture)


if __name__ == "__main__":
    unittest.main()
