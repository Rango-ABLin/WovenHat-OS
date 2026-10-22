"""Fail-closed WovenFS QEMU device acceptance tests (no disk access)."""
import importlib.util
from pathlib import Path
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / 'scripts' / 'test-wovenfs-device.py'
SPEC = importlib.util.spec_from_file_location('wovenfs_device_test', SCRIPT)
DEVICE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DEVICE)

PASS_LOG = (
    '[S15.1] WovenFS native filesystem: PASSED\n'
    '[S15.2] WovenFS physical device: PASSED\n'
)


class WovenFsDeviceEvidenceTests(unittest.TestCase):
    def test_valid_exit_and_marker_pass(self):
        self.assertEqual(DEVICE.wovenfs_device_errors(PASS_LOG, 33), [])

    def test_missing_marker_rejected(self):
        errors = DEVICE.wovenfs_device_errors('[S15.1] WovenFS native filesystem: PASSED', 33)
        self.assertTrue(any('Missing WovenFS' in error for error in errors))

    def test_wrong_exit_rejected_even_with_success_marker(self):
        errors = DEVICE.wovenfs_device_errors(PASS_LOG, 0)
        self.assertTrue(any('exit code' in error for error in errors))

    def test_fatal_evidence_after_success_rejected(self):
        for marker in ('kernel panic', 'panicked at', 'security violation', 'security: failed'):
            with self.subTest(marker=marker):
                errors = DEVICE.wovenfs_device_errors(PASS_LOG + marker.upper() + '\n', 33)
                self.assertTrue(any('fatal evidence' in error for error in errors))

    def test_contradictory_failure_marker_rejected(self):
        errors = DEVICE.wovenfs_device_errors(
            PASS_LOG + '[S15.2] WovenFS physical device: FAILED\n', 33)
        self.assertTrue(any('Contradictory' in error for error in errors))

    def test_unrelated_diagnostic_text_does_not_mask_success(self):
        self.assertEqual(
            DEVICE.wovenfs_device_errors(
                'diagnostic: intentionally rejected invalid metadata\n' + PASS_LOG, 33),
            [])


if __name__ == '__main__':
    unittest.main()
