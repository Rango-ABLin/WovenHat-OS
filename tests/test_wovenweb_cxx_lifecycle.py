"""Static contracts for the controlled native C++ global-lifecycle fixture."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


class CxxLifecycleContracts(unittest.TestCase):
    def setUp(self):
        self.main = (ROOT / "tools/wovenweb-cxx-probe/src/main.rs").read_text()
        self.linker = (ROOT / "tools/wovenweb-cxx-probe/link.x").read_text()
        self.lifecycle = (ROOT / "tools/wovenweb-cxx-probe/src/lifecycle.cpp").read_text()
        self.harness = (ROOT / "scripts/test-wovenweb-cxx-qemu.py").read_text()

    def test_init_array_is_retained_and_bounded_by_linker_symbols(self):
        self.assertIn("PROVIDE_HIDDEN(__init_array_start = .);", self.linker)
        self.assertIn("KEEP(*(SORT_BY_INIT_PRIORITY(.init_array.*)))", self.linker)
        self.assertIn("PROVIDE_HIDDEN(__init_array_end = .);", self.linker)
        self.assertIn("(end - start) / entry_size != 1", self.main)

    def test_global_object_registers_and_runs_its_destructor(self):
        self.assertIn("GlobalLifecycle lifecycle;", self.lifecycle)
        self.assertIn("__cxa_atexit", self.main)
        self.assertIn("for index in (0..count).rev()", self.main)
        self.assertIn("woven_global_destroyed() == woven_global_destroyed_marker()", self.main)

    def test_guest_marker_is_required_by_the_qemu_harness(self):
        marker = "global init-array constructor + reverse registered destructor: PASSED"
        self.assertIn(marker, self.main)
        self.assertIn(marker, self.harness)


if __name__ == "__main__":
    unittest.main()
