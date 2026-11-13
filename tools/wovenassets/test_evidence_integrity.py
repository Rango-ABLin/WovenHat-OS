"""Host-only evidence-boundary regression tests. No fabricated QEMU passes."""
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image
from qualify_guest_frame import qualify
from run_guest_visual_gate import run as run_gate


class EvidenceIntegrityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.approved = self.root / "approved.png"
        self.actual = self.root / "capture.png"
        Image.new("RGBA", (20, 12), (25, 75, 130, 255)).save(self.approved)
        Image.new("RGBA", (20, 12), (25, 75, 130, 255)).save(self.actual)
        self.manifest = self.root / "baselines.json"
        self.captures = self.root / "captures.json"
        self.baseline = {
            "id": "desktop",
            "path": "approved.png",
            "sha256": hashlib.sha256(self.approved.read_bytes()).hexdigest(),
            "size": [20, 12],
            "reviewed_by": "reviewer@example",
            "guest_commit": "example-commit",
            "capture_method": "QEMU QMP screendump",
            "max_abs": 0,
            "mean_abs": 0,
        }
        self.write_baseline()
        self.captures.write_text(json.dumps({
            "schema_version": 1,
            "captures": [{"id": "desktop", "path": "capture.png"}],
        }))

    def write_baseline(self):
        self.manifest.write_text(json.dumps({
            "schema_version": 1, "kind": "wovenhat-guest-baselines",
            "baselines": [self.baseline],
        }))

    def test_valid_reference_and_capture(self):
        self.assertTrue(run_gate(self.manifest, self.captures)["pass"])

    def test_mutating_approved_reference_rejects(self):
        Image.new("RGBA", (20, 12), "red").save(self.approved)
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            run_gate(self.manifest, self.captures)

    def test_capture_missing_rejects(self):
        self.actual.unlink()
        with self.assertRaisesRegex(ValueError, "not present"):
            run_gate(self.manifest, self.captures)

    def test_capture_pixel_difference_fails(self):
        Image.new("RGBA", (20, 12), (25, 76, 130, 255)).save(self.actual)
        self.assertFalse(run_gate(self.manifest, self.captures)["pass"])

    def test_invalid_capture_dimensions_fails(self):
        Image.new("RGBA", (19, 12), (25, 75, 130, 255)).save(self.actual)
        self.assertFalse(run_gate(self.manifest, self.captures)["pass"])

    def test_missing_approval_rejects(self):
        self.baseline["reviewed_by"] = ""
        self.write_baseline()
        with self.assertRaisesRegex(ValueError, "reviewed_by"):
            qualify(self.manifest, "desktop", self.actual)

    def test_mismatched_capture_set_rejects(self):
        self.captures.write_text(json.dumps({
            "schema_version": 1,
            "captures": [{"id": "other", "path": "capture.png"}],
        }))
        with self.assertRaises(ValueError):
            run_gate(self.manifest, self.captures)


if __name__ == "__main__":
    unittest.main()
