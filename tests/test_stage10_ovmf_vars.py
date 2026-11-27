"""Static regression tests for Stage 10 runtime OVMF pflash isolation."""
import ast
from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[1] / 'scripts' / 'test-stage10-runtime.py'


class Stage10OvmfVarsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = SOURCE.read_text(encoding='utf-8')
        cls.tree = ast.parse(cls.source)

    def test_cli_advertises_optional_vars_template(self):
        self.assertIn("'--firmware-vars'", self.source)
        self.assertIn('args.firmware_vars is not None and not args.firmware_vars.is_file()', self.source)

    def test_vars_template_copied_to_per_run_evidence(self):
        self.assertIn('def copy_firmware_vars(template: Path | None, evidence: Path) -> Path | None:', self.source)
        self.assertIn("destination = evidence / 'OVMF_VARS.fd'", self.source)
        self.assertIn('shutil.copyfile(template, destination)', self.source)
        self.assertIn('vars_copy = copy_firmware_vars(args.firmware_vars, out)', self.source)

    def test_code_is_read_only_and_vars_writable_on_separate_units(self):
        self.assertIn("def firmware_drives(code: Path, vars_copy: Path | None) -> list[str]:", self.source)
        self.assertIn('if=pflash,unit=0,format=raw,readonly=on,file={code}', self.source)
        self.assertIn('if=pflash,unit=1,format=raw,file={vars_copy}', self.source)
        self.assertNotIn('if=pflash,unit=1,format=raw,readonly=on,file={vars_copy}', self.source)
        self.assertIn('*firmware_drives(firmware, vars_copy)', self.source)


if __name__ == '__main__':
    unittest.main()
