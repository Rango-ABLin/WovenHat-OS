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

    def test_invalid_suite_and_cpu_are_rejected(self):
        with self.assertRaises(ValueError):
            MODULE.build_command("unknown", 1, 300)
        with self.assertRaises(ValueError):
            MODULE.build_command("memory", 3, 300)


if __name__ == "__main__":
    unittest.main()
