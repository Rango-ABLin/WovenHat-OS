import importlib.util
import types
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "test-stage31-randomized-faults.py"
SPEC = importlib.util.spec_from_file_location("stage31_randomized", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class Stage31RandomizedFaultTests(unittest.TestCase):
    def test_network_scenario_uses_seeded_multi_flap(self):
        args = types.SimpleNamespace(
            cpus=4,
            qemu="qemu-system-x86_64",
            firmware=Path("OVMF_CODE.fd"),
            seed=0x51A7E,
        )
        command = MODULE.scenario_command("network-loss", args)
        self.assertIn("--link-loss", command)
        self.assertEqual(command[command.index("--link-loss-count") + 1], "3")
        self.assertEqual(command[command.index("--fault-seed") + 1], "0x51a7e")
