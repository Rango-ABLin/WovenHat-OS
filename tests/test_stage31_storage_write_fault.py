import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "test-stage31-storage-write-fault.py"


class Stage31StorageWriteFaultTests(unittest.TestCase):
    def test_script_is_disposable_and_fail_closed(self):
        source = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("physical-media", source)
        self.assertIn("blkdebug:", source)
        self.assertIn('event = "write_aio"', source)
        self.assertIn('errno = "28"', source)
        self.assertIn('sector = "26"', source)
        self.assertIn("mkdir(parents=True, exist_ok=False)", source)
        self.assertIn("STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: FAILED at ", source)
        self.assertIn("guest reported success after injected write failure", source)


if __name__ == "__main__":
    unittest.main()
