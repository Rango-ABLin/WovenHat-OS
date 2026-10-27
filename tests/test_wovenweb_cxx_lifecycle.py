"""Static contracts for the controlled native C++ global-lifecycle fixture."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


class CxxLifecycleContracts(unittest.TestCase):
    def setUp(self):
        self.main = (ROOT / "tools/wovenweb-cxx-probe/src/main.rs").read_text()
        self.build = (ROOT / "tools/wovenweb-cxx-probe/build.rs").read_text()
        self.linker = (ROOT / "tools/wovenweb-cxx-probe/link.x").read_text()
        self.lifecycle = (ROOT / "tools/wovenweb-cxx-probe/src/lifecycle.cpp").read_text()
        self.lifecycle_second = (ROOT / "tools/wovenweb-cxx-probe/src/lifecycle_second.cpp").read_text()
        self.c_abi = (ROOT / "tools/wovenweb-cxx-probe/src/c_abi.c").read_text()
        self.runtime_c_abi = (ROOT / "tools/runtime-fixtures/c-abi.c").read_text()
        self.runtime_c_linker = (ROOT / "tools/runtime-fixtures/link.x").read_text()
        self.harness = (ROOT / "scripts/test-wovenweb-cxx-qemu.py").read_text()

    def test_init_array_is_retained_and_bounded_by_linker_symbols(self):
        self.assertIn("PROVIDE_HIDDEN(__init_array_start = .);", self.linker)
        self.assertIn("KEEP(*(SORT_BY_INIT_PRIORITY(.init_array.*)))", self.linker)
        self.assertIn("PROVIDE_HIDDEN(__init_array_end = .);", self.linker)
        self.assertIn("(end - start) / entry_size != 2", self.main)
        self.assertIn("for index in 0..2", self.main)
        self.assertIn('"lifecycle_second"', self.build)

    def test_global_object_registers_and_runs_its_destructor(self):
        self.assertIn("GlobalEarly lifecycle __attribute__((init_priority(200)));", self.lifecycle)
        self.assertIn("GlobalLate lifecycle_second __attribute__((init_priority(300)));", self.lifecycle_second)
        self.assertIn("phase = phase == 1 ? constructed_marker : 0;", self.lifecycle)
        self.assertIn("phase = phase == constructed_marker ? finalized_marker : 0;", self.lifecycle)
        self.assertIn("__cxa_atexit", self.main)
        self.assertIn("for index in (0..count).rev()", self.main)
        self.assertIn("woven_global_destroyed() == woven_global_destroyed_marker()", self.main)

    def test_guest_marker_is_required_by_the_qemu_harness(self):
        marker = "global init-array constructor + reverse registered destructor: PASSED"
        self.assertIn(marker, self.main)
        self.assertIn(marker, self.harness)

    def test_freestanding_c_abi_fixture_has_no_hosted_runtime_dependency(self):
        self.assertIn('"-std=c23"', self.build)
        self.assertIn('"-ffreestanding"', self.build)
        self.assertIn("extern wh_u64 woven_mix", self.c_abi)
        self.assertIn("extern struct wh_pair woven_pair", self.c_abi)
        marker = "freestanding C ABI + aggregate return + data: PASSED"
        self.assertIn(marker, self.main)
        self.assertIn(marker, self.harness)

    def test_standalone_c_fixture_uses_its_own_entry_and_is_required_by_qemu(self):
        self.assertIn("void _start", self.runtime_c_abi)
        self.assertIn("int $0x80", self.runtime_c_abi)
        self.assertIn("ENTRY(_start)", self.runtime_c_linker)
        self.assertIn("WOVENHAT_RUNTIME_C_ABI_ELF", self.harness)
        self.assertIn("rust_lld()", self.harness)
        self.assertIn('"-flavor",', self.harness)
        marker = "freestanding C _start + exit ABI: PASSED"
        self.assertIn(marker, self.harness)


if __name__ == "__main__":
    unittest.main()
