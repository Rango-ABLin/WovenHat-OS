import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "test-stage31-repeated-boots.py"
SPEC = importlib.util.spec_from_file_location("stage31_repeat", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class Stage31RepeatedBootsTests(unittest.TestCase):
    def test_device_suites_receive_timeout(self):
        command = MODULE.build_command("storage", 16, 900)
        self.assertEqual(command[-2:], ["--timeout", "900"])

    def test_shell_uses_its_own_bounded_harness(self):
        command = MODULE.build_command("shell", 8, 300)
        self.assertNotIn("--timeout", command)
        self.assertEqual(command[-2:], ["--cpus", "8"])

    def test_recovery_uses_recovery_gate_and_cpu_limit(self):
        command = MODULE.build_command("recovery", 2, 300)
        self.assertIn("scripts/test-stage27-5-recovery.py", command)
        self.assertEqual(command[-2:], ["--timeout", "300"])
        with self.assertRaises(ValueError):
            MODULE.build_command("recovery", 8, 300)

    def test_update_service_uses_signed_service_gate_and_cpu_limit(self):
        command = MODULE.build_command("update-service", 4, 300)
        self.assertIn("scripts/test-stage27-5-update-service.py", command)
        self.assertEqual(command[-2:], ["--timeout", "300"])
        with self.assertRaises(ValueError):
            MODULE.build_command("update-service", 16, 300)

    def test_invalid_suite_and_cpu_are_rejected(self):
        with self.assertRaises(ValueError):
            MODULE.build_command("unknown", 1, 300)
        with self.assertRaises(ValueError):
            MODULE.build_command("memory", 3, 300)

    def test_count_qualified_evidence_preserves_ten_boot_sample(self):
        self.assertTrue(str(MODULE.evidence_directory("memory", 1, 10)).endswith("memory-1cpu"))
        self.assertTrue(str(MODULE.evidence_directory("memory", 1, 100)).endswith("memory-1cpu-100boots"))


if __name__ == "__main__":
    unittest.main()
