"""Contract tests for Stage 13.12 OVMF firmware isolation (no QEMU or disks)."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "test-stage13-12-drivers.py"
SPEC = importlib.util.spec_from_file_location("stage13_12_drivers", SCRIPT)
DRIVERS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVERS)


class Stage1312FirmwareTests(unittest.TestCase):
    def test_without_vars_preserves_readonly_code_drive(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            code = root / "CODE.fd"
            code.write_bytes(b"code")
            drives = DRIVERS.firmware_drive_args(code, None, root)
            self.assertEqual(
                drives,
                ["-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={code}"],
            )
            self.assertFalse((root / "OVMF_VARS.fd").exists())

    def test_vars_are_copied_per_run_and_source_is_untouched(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            code, template = root / "CODE.fd", root / "VARS.fd"
            code.write_bytes(b"code")
            template.write_bytes(b"original-vars")
            first, second = root / "run1", root / "run2"
            first.mkdir()
            second.mkdir()
            one = DRIVERS.firmware_drive_args(code, template, first)
            (first / "OVMF_VARS.fd").write_bytes(b"mutated-by-guest")
            two = DRIVERS.firmware_drive_args(code, template, second)
            self.assertEqual(template.read_bytes(), b"original-vars")
            self.assertEqual((second / "OVMF_VARS.fd").read_bytes(), b"original-vars")
            self.assertIn("readonly=on", one[1])
            self.assertIn("unit=1", one[3])
            self.assertNotIn("readonly=on", one[3])
            self.assertIn(str(first / "OVMF_VARS.fd"), one[3])
            self.assertIn(str(second / "OVMF_VARS.fd"), two[3])

    def test_missing_vars_template_fails_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            with self.assertRaises(FileNotFoundError):
                DRIVERS.firmware_drive_args(root / "CODE.fd", root / "missing.fd", root)

    def test_cli_documents_release_firmware_vars_option(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--help"],
            capture_output=True,
            text=True,
            check=True,
        )
        self.assertIn("--firmware-vars", result.stdout)


if __name__ == "__main__":
    unittest.main()
