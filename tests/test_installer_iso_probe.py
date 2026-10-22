import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).parents[1] / "scripts" / "probe-installer-iso.py"
SPEC = importlib.util.spec_from_file_location("installer_iso_probe", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def disk_image(path: Path) -> None:
    data = bytearray(4096)
    data[510:512] = b"\x55\xaa"
    data[512:520] = b"EFI PART"
    path.write_bytes(data)


class InstallerIsoProbeTests(unittest.TestCase):
    def test_current_missing_capabilities_are_explicit_and_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "wovenhat.img"
            disk_image(source)
            with mock.patch.object(MODULE.shutil, "which", return_value=None):
                result = MODULE.probe(source, None)
            self.assertFalse(result["iso_supported"])
            self.assertEqual(result["status"], "blocked")
            self.assertIn("no supported ISO builder is installed (searched: xorriso, mkisofs, genisoimage, oscdimg)", result["blockers"])
            self.assertIn("required ISO boot file EFI/BOOT/BOOTX64.EFI cannot be verified", result["blockers"])
            self.assertFalse(result["safety"]["writes_artifacts"])

    def test_invalid_disk_image_is_rejected_without_mounting(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "not-a-disk.img"
            source.write_bytes(b"bad")
            with mock.patch.object(MODULE.shutil, "which", return_value="C:\\tools\\xorriso.exe"):
                result = MODULE.probe(source, None)
            self.assertEqual(result["status"], "blocked")
            self.assertIn("source image is not a non-zero 512-byte sector multiple", result["blockers"])

    def test_prerequisites_are_not_support_claim(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "wovenhat.img"
            boot = root / "EFI" / "BOOT" / "BOOTX64.EFI"
            boot.parent.mkdir(parents=True)
            boot.write_bytes(b"UEFI")
            disk_image(source)
            with mock.patch.object(MODULE.shutil, "which", return_value="C:\\tools\\xorriso.exe"):
                result = MODULE.probe(source, root)
            self.assertEqual(result["status"], "ready-for-build-validation")
            self.assertFalse(result["iso_supported"])
            self.assertEqual(result["safety"]["qemu_boot_validation"], "not-run")


if __name__ == "__main__":
    unittest.main()
