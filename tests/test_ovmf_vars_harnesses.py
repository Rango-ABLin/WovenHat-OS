"""Regression: every release-validation app harness accepts OVMF VARS arguments.

Run: python3 -m unittest discover -s tests -p 'test_ovmf_vars_harnesses.py'
"""
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HARNESSES = (
    "monitor", "settings", "security", "editor", "images", "update",
    "system-app-catalog",
)


class OvmfVarsHarnessTests(unittest.TestCase):
    def test_release_harnesses_advertise_firmware_vars(self):
        for name in HARNESSES:
            with self.subTest(harness=name):
                script = ROOT / "scripts" / f"test-{name}-qemu.py"
                proc = subprocess.run(
                    [sys.executable, str(script), "--help"],
                    capture_output=True, text=True, check=False,
                )
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assertIn("--firmware-vars", proc.stdout)

    def test_firmware_vars_are_copied_and_attached_as_writable_pflash(self):
        for name in HARNESSES:
            with self.subTest(harness=name):
                source = (ROOT / "scripts" / f"test-{name}-qemu.py").read_text()
                self.assertIn("shutil.copyfile(firmware_vars, vars_copy)", source)
                self.assertIn("if=pflash,unit=1,format=raw,file={vars_copy}", source)
                self.assertIn("if=pflash,unit=0,format=raw,readonly=on,file={firmware}", source)


if __name__ == "__main__":
    unittest.main()
