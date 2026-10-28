"""Verify the production userspace allocator initialization protocol."""
from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "libwoven/src/lib.rs"

class AllocatorProtocolTests(unittest.TestCase):
    def test_initializer_ownership_and_publication(self):
        source = SOURCE.read_text(encoding="utf-8")
        begin = source.index("mod userspace_allocator {")
        end = source.index("\npub mod process {", begin)
        allocator = source[begin:end]
        self.assertIn("compare_exchange(0, u64::MAX", allocator)
        self.assertIn("if next == u64::MAX", allocator)
        self.assertIn("checked_add(ARENA_BYTES)", allocator)
        self.assertLess(
            allocator.index("END.store(bound, Ordering::Release)"),
            allocator.index("NEXT.store(base, Ordering::Release)"),
        )
        self.assertIn("NEXT.store(0, Ordering::Release)", allocator)

if __name__ == "__main__":
    unittest.main()
