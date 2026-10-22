"""Negative regressions for WovenMonitor QEMU acceptance evidence."""
import importlib.util
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "test-monitor-qemu.py"
SPEC = importlib.util.spec_from_file_location("woven_monitor_qemu", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
PASS = "[S16.1] out-of-kernel application: PASSED"


class MonitorEvidenceTests(unittest.TestCase):
    def test_clean_exact_marker_and_expected_exit_pass(self):
        self.assertTrue(MODULE.validate_monitor_result(33, "Booted\n" + PASS + "\n"))

    def test_wrong_exit_rejected(self):
        self.assertFalse(MODULE.validate_monitor_result(35, PASS))

    def test_substring_and_duplicate_markers_rejected(self):
        self.assertFalse(MODULE.validate_monitor_result(33, "prefix " + PASS + "\n"))
        self.assertFalse(MODULE.validate_monitor_result(33, PASS + "\n" + PASS + "\n"))

    def test_contradictory_failure_and_panic_rejected(self):
        for failure in ("[S16.1] out-of-kernel application: FAILED",
                        "kernel panic: invalid state", "panicked at kernel/src/main.rs",
                        "assertion failed: x", "SECURITY VIOLATION"):
            with self.subTest(failure=failure):
                self.assertFalse(MODULE.validate_monitor_result(33, PASS + "\n" + failure))

    def test_missing_marker_rejected(self):
        self.assertFalse(MODULE.validate_monitor_result(33, "Booted normally"))

    def test_unique_evidence_and_collision_refusal(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            first = MODULE.create_evidence_directory(root, 2, 123)
            manifest = first / "manifest.json"
            manifest.write_text("original", encoding="utf-8")
            with self.assertRaises(FileExistsError):
                MODULE.create_evidence_directory(root, 2, 123)
            self.assertEqual(manifest.read_text(encoding="utf-8"), "original")
            second = MODULE.create_evidence_directory(root, 2, 124)
            self.assertNotEqual(first, second)
            self.assertTrue(second.is_dir())


if __name__ == "__main__":
    unittest.main()
