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
        code = Path("CODE.fd")
        vars_copy = Path("run/OVMF_VARS.fd")
        drives = RUNTIME.firmware_drives(code, vars_copy)
        self.assertEqual(drives, [
            "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={code}",
            "-drive", f"if=pflash,unit=1,format=raw,file={vars_copy}",
        ])
        self.assertNotIn(f"readonly=on,file={vars_copy}", " ".join(drives))

    def test_invalid_template_does_not_create_vars_copy(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaises(FileNotFoundError):
                RUNTIME.copy_firmware_vars(root / "missing.fd", root)
            self.assertFalse((root / "OVMF_VARS.fd").exists())


if __name__ == "__main__":
    unittest.main()
