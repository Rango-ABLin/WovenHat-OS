"""Host regression tests for the GFX-07 fail-closed output detector."""
import importlib.util
import pathlib
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts" / "test-gfx07-qemu-matrix.py"
spec = importlib.util.spec_from_file_location("gfx07_matrix", SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class FatalMarkers(unittest.TestCase):
    def test_rejects_failed_and_timeout_markers(self):
        for line in (
            "[S20.14B] desktop across monitors: FAILED",
            "[S20.15] session TIMEOUT",
            "kernel panic: unexpected state",
            "panicked at kernel/src/display.rs",
            "security violation",
        ):
            with self.subTest(line=line):
                self.assertIsNotNone(module.FATAL.search(line))

    def test_allows_success_and_incidental_words(self):
        for line in (
            "[S20.14B] desktop across monitors: PASSED",
            "graphics fallback activated safely",
            "failure-free rendering path",
            "timeout_ns setting is 900",
            "staged frame presented",
        ):
            with self.subTest(line=line):
                self.assertIsNone(module.FATAL.search(line))


if __name__ == "__main__":
    unittest.main()
