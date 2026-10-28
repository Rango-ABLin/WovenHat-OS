import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "test-stage31-low-ram.py"
SPEC = importlib.util.spec_from_file_location("stage31_low_ram", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class Stage31LowRamTests(unittest.TestCase):
    def test_profile_is_the_existing_disposable_low_memory_fixture(self):
        self.assertEqual(MODULE.PROFILES, ("256M",))

    def test_command_forwards_memory_and_firmware(self):
        command = MODULE.build_command(4, "256M", "qemu", Path("OVMF_CODE.fd"), 180)
        self.assertEqual(command[command.index("--cpus") + 1], "4")
        self.assertEqual(command[command.index("--memory") + 1], "256M")
        self.assertEqual(command[command.index("--qemu") + 1], "qemu")
        self.assertEqual(command[command.index("--firmware") + 1], "OVMF_CODE.fd")
        self.assertEqual(command[-2:], ["--timeout", "180"])


if __name__ == "__main__":
    unittest.main()
