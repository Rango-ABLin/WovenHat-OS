import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest


ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts" / "extract-uefi-boot-artifact.py"
SPEC = importlib.util.spec_from_file_location("extract_uefi_boot_artifact", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def short_entry(name: bytes, attributes: int, cluster: int, size: int = 0) -> bytes:
    entry = bytearray(32)
    entry[:11] = name.ljust(11, b" ")
    entry[11] = attributes
    struct.pack_into("<H", entry, 26, cluster)
    struct.pack_into("<I", entry, 28, size)
    return bytes(entry)


def make_fat16_esp(payload: bytes) -> bytes:
    sectors = 4096
    reserved = 1
    fat_sectors = 16
    root_entries = 64
    root_sectors = 4
    data_start = (reserved + 2 * fat_sectors + root_sectors) * MODULE.SECTOR
    esp = bytearray(sectors * MODULE.SECTOR)
    esp[0:3] = b"\xeb<\x90"
    struct.pack_into("<H B H B H H B H H H I I", esp, 11, 512, 1, reserved, 2, root_entries, sectors, 0xF8, fat_sectors, 63, 255, 0, 0)
    esp[510:512] = b"\x55\xaa"
    for fat_offset in (reserved * MODULE.SECTOR, (reserved + fat_sectors) * MODULE.SECTOR):
        struct.pack_into("<HHHHH", esp, fat_offset, 0xFFF8, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF)
    root = data_start - root_sectors * MODULE.SECTOR
    esp[root : root + 32] = short_entry(b"EFI", 0x10, 2)
    cluster2 = data_start
    esp[cluster2 : cluster2 + 32] = short_entry(b".", 0x10, 2)
    esp[cluster2 + 32 : cluster2 + 64] = short_entry(b"..", 0x10, 0)
    esp[cluster2 + 64 : cluster2 + 96] = short_entry(b"BOOT", 0x10, 3)
    cluster3 = data_start + MODULE.SECTOR
    esp[cluster3 : cluster3 + 32] = short_entry(b".", 0x10, 3)
    esp[cluster3 + 32 : cluster3 + 64] = short_entry(b"..", 0x10, 2)
    entry = bytearray(32)
    entry[:8] = b"BOOTX64 "
    entry[8:11] = b"EFI"
    entry[11] = 0x20
    struct.pack_into("<H", entry, 26, 4)
    struct.pack_into("<I", entry, 28, len(payload))
    esp[cluster3 + 64 : cluster3 + 96] = entry
    esp[data_start + 2 * MODULE.SECTOR : data_start + 2 * MODULE.SECTOR + len(payload)] = payload
    return bytes(esp)


class ExtractUefiArtifactTests(unittest.TestCase):
    def test_reads_case_insensitive_fat16_fallback_path(self):
        payload = b"MZ" + bytes(range(64))
        view = MODULE.FatView(make_fat16_esp(payload))
        self.assertEqual(view.find(("EFI", "BOOT", "BOOTX64.EFI")), payload)

    def test_rejects_cluster_loops(self):
        esp = bytearray(make_fat16_esp(b"payload"))
        fat = MODULE.SECTOR
        struct.pack_into("<H", esp, fat + 4 * 2, 4)
        with self.assertRaisesRegex(MODULE.ExtractError, "loops"):
            MODULE.FatView(bytes(esp)).find(("EFI", "BOOT", "BOOTX64.EFI"))


if __name__ == "__main__":
    unittest.main()
