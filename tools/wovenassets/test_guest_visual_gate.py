import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from PIL import Image
from run_guest_visual_gate import run


class GateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.baseline = self.root / "ref.png"
        self.capture = self.root / "actual.png"
        Image.new("RGBA", (16, 12), (50, 20, 10, 255)).save(self.baseline)
        Image.new("RGBA", (16, 12), (50, 20, 10, 255)).save(self.capture)
        self.manifest = self.root / "baseline.json"
        self.map = self.root / "captures.json"
        self.manifest.write_text(json.dumps({
            "schema_version": 1, "kind": "wovenhat-guest-baselines",
            "baselines": [{
                "id": "desktop", "path": "ref.png",
                "sha256": hashlib.sha256(self.baseline.read_bytes()).hexdigest(),
                "size": [16, 12], "reviewed_by": "graphics-reviewer",
                "guest_commit": "example-sha", "capture_method": "qemu-screendump",
                "max_abs": 0, "mean_abs": 0.0
            }]
        }))
        self.write_map([{"id": "desktop", "path": "actual.png"}])

    def write_map(self, entries):
        self.map.write_text(json.dumps({"schema_version": 1, "captures": entries}))

    def test_verified_guest_capture(self):
        self.assertTrue(run(self.manifest, self.map)["pass"])

    def test_changed_guest_capture(self):
        Image.new("RGBA", (16, 12), (51, 20, 10, 255)).save(self.capture)
        self.assertFalse(run(self.manifest, self.map)["pass"])

    def test_capture_missing(self):
        self.capture.unlink()
        with self.assertRaisesRegex(ValueError, "not present"):
            run(self.manifest, self.map)

    def test_empty_capture_map_fails_closed(self):
        self.write_map([])
        with self.assertRaisesRegex(ValueError, "empty"):
            run(self.manifest, self.map)

    def test_unreviewed_capture_cannot_satisfy_gate(self):
        self.write_map([{"id": "other", "path": "actual.png"}])
        with self.assertRaises(ValueError):
            run(self.manifest, self.map)

    def test_duplicate_capture_fails(self):
        entry = {"id": "desktop", "path": "actual.png"}
        self.write_map([entry, entry])
        with self.assertRaisesRegex(ValueError, "duplicate"):
            run(self.manifest, self.map)

    def test_path_traversal_fails(self):
        self.write_map([{"id": "desktop", "path": "../actual.png"}])
        with self.assertRaisesRegex(ValueError, "unsafe"):
            run(self.manifest, self.map)


if __name__ == "__main__":
    unittest.main()
