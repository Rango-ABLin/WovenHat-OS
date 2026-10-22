"""Host tests for the disposable UEFI ISO builder (unittest, like the rest of tests/)."""
import importlib.util
import struct
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[1]
SPEC = importlib.util.spec_from_file_location("build_uefi_iso", ROOT / "scripts" / "build-uefi-iso.py")
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def _source(path: Path) -> None:
    sector = 512
    image = bytearray(256 * sector)
    image[510:512] = b"\x55\xaa"
    header = bytearray(sector)
    header[:8] = b"EFI PART"
    struct.pack_into("<QII", header, 72, 2, 4, 128)
    image[sector : 2 * sector] = header
    entry = bytearray(128)
    entry[:16] = MODULE.ESP_GUID
    struct.pack_into("<QQ", entry, 32, 32, 127)
    image[2 * sector : 2 * sector + 128] = entry
    esp = memoryview(image)[32 * sector : 128 * sector]
    esp[510:512] = b"\x55\xaa"
    esp[82:90] = b"FAT32   "
    esp[120:131] = b"BOOTX64.EFI"
    path.write_bytes(image)


class UefiIsoBuilderTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp_path = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_builds_el_torito_iso_from_disposable_gpt(self) -> None:
        source = self.tmp_path / "source.img"
        output = self.tmp_path / "wovenhat.iso"
        _source(source)
        result = MODULE.build_iso(source, output)
        raw = output.read_bytes()
        self.assertEqual(result["format"], "ISO9660-El-Torito-UEFI")
        self.assertEqual(raw[16 * 2048 + 1 : 16 * 2048 + 6], b"CD001")
        self.assertEqual(raw[17 * 2048 + 1 : 17 * 2048 + 6], b"CD001")
        # Volume descriptor set terminator (type 255) closes the set at LBA 18.
        self.assertEqual(raw[18 * 2048], 255)
        self.assertEqual(raw[18 * 2048 + 1 : 18 * 2048 + 6], b"CD001")
        self.assertEqual(raw[20 * 2048 + 32], 0x91)
        self.assertEqual(raw[21 * 2048 + 510 : 21 * 2048 + 512], b"\x55\xaa")

    def test_refuses_protected_paths(self) -> None:
        source = self.tmp_path / "source.img"
        _source(source)
        with self.assertRaisesRegex(MODULE.IsoError, "protected"):
            MODULE.build_iso(source, self.tmp_path / "backup-disk.iso")

    def test_refuses_missing_esp(self) -> None:
        source = self.tmp_path / "source.img"
        source.write_bytes(b"\0" * (256 * 512))
        with self.assertRaisesRegex(MODULE.IsoError, "GPT header"):
            MODULE.build_iso(source, self.tmp_path / "out.iso")


if __name__ == "__main__":
    unittest.main()
