"""A negative-control gate must not mistake build/fatal failures for evidence."""
import importlib.util
from pathlib import Path
import unittest
spec = importlib.util.spec_from_file_location("generation_baseline", Path(__file__).resolve().parents[1] / "scripts/test-wovenweb-generation-baseline.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
class ObservationTests(unittest.TestCase):
    def setUp(self):
        self.serial = "\n".join([module.MARKER, "remap phase=old-fault-queued", "remap phase=ready-file-published", "queued sibling page deltas + duplicate coalescing + exact cleanup: PASSED"])
    def test_precise_old_behavior_is_accepted(self):
        module.verify_observation({"passed": False, "exit": 35}, 1, self.serial)
    def test_unrelated_or_build_failures_are_rejected(self):
        for serial in ["compiler error", "KERNEL PANIC", "", self.serial.replace(module.MARKER, "wrong resident count")]:
            with self.assertRaises(ValueError): module.verify_observation({"passed": False, "exit": 35}, 1, serial)
    def test_positive_guest_and_timeout_status_are_rejected(self):
        for result, status in [({"passed": True, "exit": 33}, 0), ({"passed": False, "exit": -1}, 1), ({"passed": False, "exit": 35}, 124)]:
            with self.assertRaises(ValueError): module.verify_observation(result, status, self.serial)
    def test_missing_handshake_is_rejected(self):
        with self.assertRaises(ValueError): module.verify_observation({"passed": False, "exit": 35}, 1, module.MARKER)
if __name__ == "__main__": unittest.main()
