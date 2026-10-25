import math
import tempfile
import unittest
from pathlib import Path
from PIL import Image
from compare_frames import compare


class ComparisonHardeningTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.reference = self.root / "ref.png"
        self.capture = self.root / "guest.png"
        self.diff = self.root / "diff.png"
        Image.new("RGBA", (12, 8), (12, 18, 24, 255)).save(self.reference)
        Image.new("RGBA", (12, 8), (12, 18, 24, 255)).save(self.capture)

    def test_reject_negative_tolerance(self):
        with self.assertRaises(ValueError):
            compare(self.reference, self.capture, max_abs=-1)

    def test_reject_nan_and_infinite_tolerance(self):
        for threshold in [math.nan, math.inf, -math.inf]:
            with self.subTest(threshold=threshold):
                with self.assertRaises(ValueError):
                    compare(self.reference, self.capture, mean_abs=threshold)

    def test_reject_invalid_types(self):
        with self.assertRaises(ValueError):
            compare(self.reference, self.capture, max_abs=True)
        with self.assertRaises(ValueError):
            compare(self.reference, self.capture, mean_abs="0")

    def test_remove_stale_diff_on_dimension_mismatch(self):
        self.diff.write_bytes(b"stale")
        Image.new("RGBA", (11, 8), (12, 18, 24, 255)).save(self.capture)
        result = compare(self.reference, self.capture, out=self.diff)
        self.assertFalse(result["pass"])
        self.assertEqual(result["reason"], "dimension_mismatch")
        self.assertFalse(self.diff.exists())

    def test_remove_stale_diff_on_corrupted_capture(self):
        self.diff.write_bytes(b"stale")
        self.capture.write_bytes(b"invalid png")
        with self.assertRaises(OSError):
            compare(self.reference, self.capture, out=self.diff)
        self.assertFalse(self.diff.exists())


if __name__ == "__main__":
    unittest.main()
