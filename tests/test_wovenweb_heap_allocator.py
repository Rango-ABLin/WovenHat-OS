"""Static contracts for the opt-in WovenWeb reclaiming allocator."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


class WovenWebHeapAllocatorContracts(unittest.TestCase):
    def setUp(self):
        self.lib = (ROOT / "libwoven/src/lib.rs").read_text(encoding="utf-8")
        self.heap_allocator = (ROOT / "libwoven/src/heap_allocator.rs").read_text(
            encoding="utf-8"
        )
        self.probe_heap = (ROOT / "tools/wovenweb-net-probe/src/heap.rs").read_text(
            encoding="utf-8"
        )

    def test_legacy_arena_is_disabled_for_reclaiming_guests(self):
        self.assertIn(
            '#[cfg(not(feature = "reclaiming-allocator"))]\n'
            "    #[global_allocator]\n"
            "    static ALLOCATOR: Arena = Arena;",
            self.lib,
        )

    def test_runtime_heap_is_the_native_global_allocator(self):
        self.assertIn(
            '#[cfg(all(feature = "reclaiming-allocator", target_os = "none"))]\n'
            "#[global_allocator]\n"
            "pub(crate) static USER_HEAP",
            self.heap_allocator,
        )

    def test_standard_vec_fixture_keeps_its_full_reservation(self):
        # The regression was fixed by restoring the allocator, not by shrinking
        # the standard-library allocation the fixture requires.
        self.assertIn("try_reserve_exact(20000)", self.probe_heap)


if __name__ == "__main__":
    unittest.main()
