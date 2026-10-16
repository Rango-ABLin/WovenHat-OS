"""Regression guards for OVMF VARS isolation and Stage 13.16 QMP shutdown."""
import ast
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class HarnessRegression(unittest.TestCase):
    def test_ring3_harnesses_isolate_vars(self):
        for name in ("monitor", "settings"):
            with self.subTest(harness=name):
                source = (ROOT / "scripts" / f"test-{name}-qemu.py").read_text()
                self.assertIn("'--firmware-vars'", source)
                self.assertIn("shutil.copyfile(firmware_vars, vars_copy)", source)
                self.assertIn("if=pflash,unit=0,format=raw,readonly=on", source)
                self.assertIn("if=pflash,unit=1,format=raw,file=", source)
                self.assertIn("code != 33", source)
                self.assertIn("[S16.1] out-of-kernel application: PASSED", source)

    def test_hid_qmp_disconnect_never_bypasses_acceptance(self):
        source = (ROOT / "scripts" / "test-stage13-16-hid-reports.py").read_text()
        ast.parse(source)
        self.assertIn("raise EOFError", source)
        self.assertIn("except (OSError, EOFError):", source)
        self.assertIn("process.wait(timeout=2)", source)
        self.assertIn("process.returncode != 33 or PASS_MARKER not in completed_log", source)
        self.assertIn("if process.returncode != 33:", source)
        self.assertIn("if PASS_MARKER not in log:", source)


if __name__ == "__main__":
    unittest.main()
