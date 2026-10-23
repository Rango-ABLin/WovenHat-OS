import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "test-network-qemu.py"
sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location("stage31_network", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class Stage31NetworkFaultPlanTests(unittest.TestCase):
    def test_seeded_plan_is_reproducible_and_bounded(self):
        first = MODULE.build_link_fault_plan(0x51A7E, 8)
        second = MODULE.build_link_fault_plan(0x51A7E, 8)
        self.assertEqual(first, second)
        self.assertEqual(len(first), 8)
        self.assertTrue(all(100 <= delay <= 500 for delay in first))

    def test_plan_rejects_unbounded_counts(self):
        with self.assertRaises(ValueError):
            MODULE.build_link_fault_plan(1, 0)
        with self.assertRaises(ValueError):
            MODULE.build_link_fault_plan(1, 9)
