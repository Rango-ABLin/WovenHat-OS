"""Stage 20.15 OVMF firmware state isolation regression tests."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts' / 'test-stage20-15-session.py'
SPEC = importlib.util.spec_from_file_location('stage20_session', SCRIPT)
SESSION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SESSION)


class FirmwareIsolationTests(unittest.TestCase):
    def test_missing_optional_vars_uses_readonly_code_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            code = Path(tmp) / 'OVMF_CODE.fd'
            self.assertIsNone(SESSION.prepare_firmware_vars(None, Path(tmp)))
            self.assertEqual(SESSION.firmware_drives(code, None), [
                '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={code}',
            ])

    def test_private_vars_copy_does_not_mutate_template(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            template = base / 'OVMF_VARS.fd'
            template.write_bytes(b'original-firmware-state')
            run1 = base / 'run1'
            run2 = base / 'run2'
            run1.mkdir()
            run2.mkdir()
            copy1 = SESSION.prepare_firmware_vars(template, run1)
            copy2 = SESSION.prepare_firmware_vars(template, run2)
            self.assertNotEqual(copy1, copy2)
            copy1.write_bytes(b'mutated-by-guest')
            self.assertEqual(template.read_bytes(), b'original-firmware-state')
            self.assertEqual(copy2.read_bytes(), b'original-firmware-state')

    def test_code_readonly_and_only_private_vars_writable(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            template = base / 'template.fd'
            template.write_bytes(b'vars')
            output = base / 'run'
            output.mkdir()
            copy = SESSION.prepare_firmware_vars(template, output)
            code = base / 'code.fd'
            self.assertEqual(SESSION.firmware_drives(code, copy), [
                '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={code}',
                '-drive', f'if=pflash,unit=1,format=raw,file={copy}',
            ])
            self.assertNotIn(str(template), ' '.join(SESSION.firmware_drives(code, copy)))

    def test_nonexistent_vars_template_fails_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            with self.assertRaises(FileNotFoundError):
                SESSION.prepare_firmware_vars(base / 'missing.fd', base)


if __name__ == '__main__':
    unittest.main()
