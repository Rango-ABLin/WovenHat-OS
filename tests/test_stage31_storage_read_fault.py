import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "test-stage31-storage-read-fault.py"
sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location("stage31_storage_read_fault", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class Stage31StorageReadFaultTests(unittest.TestCase):
    def test_script_is_disposable_and_uses_blkdebug(self):
        source = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("physical disk", source)
        self.assertIn("blkdebug:", source)
        self.assertIn('event = "read_aio"', source)
        self.assertIn('errno = "5"', source)
        self.assertIn("STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: PASSED", source)
        self.assertIn("STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: SKIPPED", source)
