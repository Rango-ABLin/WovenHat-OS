"""Focused contracts for the installed-disk QEMU release gate."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "test-installed-disk-qemu.py"
SPEC = importlib.util.spec_from_file_location("installed_disk_qemu", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def make_disk(path: Path, size: int = 32 * 1024 * 1024) -> None:
    with path.open("wb") as stream:
        stream.truncate(size)
    with path.open("r+b") as stream:
        mbr = bytearray(512)
        mbr[510:512] = b"\x55\xaa"
        stream.write(mbr)
        gpt = bytearray(512)
        gpt[:8] = b"EFI PART"
        stream.write(gpt)


class InstalledDiskQemuContracts(unittest.TestCase):
    def test_rejects_physical_and_device_paths(self):
        for raw in (r"\\.\PhysicalDrive0", r"C:\\physicaldrive0", "/dev/sda"):
            with self.assertRaises(MODULE.InstalledDiskGateError):
                MODULE.regular_file(Path(raw), "disk")

    def test_rejects_symlink_and_non_regular_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            directory_path = root / "directory"
            directory_path.mkdir()
            with self.assertRaises(MODULE.InstalledDiskGateError):
                MODULE.validate_source_image(directory_path)
            source = root / "source.img"
            make_disk(source)
            link = root / "link.img"
            try:
                link.symlink_to(source)
            except (OSError, NotImplementedError):
                self.skipTest("symlinks unavailable")
            with self.assertRaises(MODULE.InstalledDiskGateError):
                MODULE.validate_source_image(link)

    def test_install_is_exact_and_atomic_file_backed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.img"
            target = root / "evidence" / "installed.img"
            make_disk(source)
            result = MODULE.install_exact_disk_image(source, target)
            self.assertTrue(result["exact_digest_match"])
            self.assertEqual(result["install_mode"], "atomic-file-backed-exact-uefi-image-copy")
            self.assertEqual(MODULE.sha256(source), MODULE.sha256(target))
            self.assertFalse(list(target.parent.glob("*.partial")))
            with self.assertRaises(MODULE.InstalledDiskGateError):
                MODULE.install_exact_disk_image(source, target)

    def test_qemu_command_contains_only_installed_disk(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            qemu = root / "qemu.exe"
            firmware = root / "OVMF_CODE.fd"
            disk = root / "installed.img"
            for path in (qemu, firmware):
                path.write_bytes(b"tool")
            make_disk(disk)
            command = MODULE.build_qemu_command(qemu, firmware, None, disk, root / "serial.log", root / "qmp.sock", 4)
            joined = " ".join(command)
            self.assertIn("-smp 4", joined)
            self.assertIn(str(disk), joined)
            self.assertNotIn("media", joined.lower())
            self.assertEqual(command.count("-drive"), 2)
            self.assertIn("readonly=on", joined)
            self.assertNotIn("-no-shutdown", command)

    def test_guest_execution_requires_uefi_kernel_and_debug_exit(self):
        good = MODULE.guest_execution_status(
            33, "INFO : UEFI bootloader started\n[BOOT] ALL VALIDATIONS PASSED\n"
        )
        self.assertTrue(good["guest_execution"])
        for code, serial in (
            (0, "INFO : UEFI bootloader started\n[BOOT] ALL VALIDATIONS PASSED\n"),
            (33, "INFO : UEFI bootloader started\n"),
            (33, "[BOOT] ALL VALIDATIONS PASSED\n"),
        ):
            self.assertFalse(MODULE.guest_execution_status(code, serial)["guest_execution"])

    def test_only_requested_cpu_profiles_are_accepted(self):
        with self.assertRaises(MODULE.InstalledDiskGateError):
            MODULE.build_qemu_command(
                Path("qemu"), Path("OVMF"), None, Path("disk"), Path("serial"), Path("qmp"), 8
            )


if __name__ == "__main__":
    unittest.main()
