"""Behavioral tests for per-run OVMF firmware-state isolation."""
import importlib.util
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "test-stage10-runtime.py"
SPEC = importlib.util.spec_from_file_location("stage10_runtime", SCRIPT)
RUNTIME = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNTIME)


class Stage10FirmwareVarsTests(unittest.TestCase):
    def test_missing_template_leaves_code_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertIsNone(RUNTIME.copy_firmware_vars(None, Path(tmp)))
            self.assertEqual(RUNTIME.firmware_drives(Path("CODE.fd"), None),
                             ["-drive", "if=pflash,unit=0,format=raw,readonly=on,file=CODE.fd"])

    def test_vars_copy_is_private_to_each_run(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            template = root / "VARS-template.fd"
            template.write_bytes(b"original")
            first = root / "first"
            second = root / "second"
            first.mkdir()
            second.mkdir()
            a = RUNTIME.copy_firmware_vars(template, first)
            b = RUNTIME.copy_firmware_vars(template, second)
            self.assertEqual(a.read_bytes(), b"original")
            a.write_bytes(b"modified")
            self.assertEqual(b.read_bytes(), b"original")
            self.assertEqual(template.read_bytes(), b"original")

    def test_code_readonly_and_vars_writable_separate_units(self):
        drives = RUNTIME.firmware_drives(Path("CODE.fd"), Path("run/OVMF_VARS.fd"))
        self.assertEqual(drives, [
            "-drive", "if=pflash,unit=0,format=raw,readonly=on,file=CODE.fd",
            "-drive", "if=pflash,unit=1,format=raw,file=run/OVMF_VARS.fd",
        ])
        self.assertNotIn("readonly=on,file=run/OVMF_VARS.fd", " ".join(drives))

    def test_invalid_template_does_not_create_vars_copy(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaises(FileNotFoundError):
                RUNTIME.copy_firmware_vars(root / "missing.fd", root)
            self.assertFalse((root / "OVMF_VARS.fd").exists())


if __name__ == "__main__":
    unittest.main()
