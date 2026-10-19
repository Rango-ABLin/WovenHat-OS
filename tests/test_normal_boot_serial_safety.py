"""Fail-closed normal-boot serial evidence tests."""
import importlib.util
from pathlib import Path
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts' / 'test-shell-qemu.py'
SPEC = importlib.util.spec_from_file_location('shell_qemu', SCRIPT)
SHELL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SHELL)

BASE = ('[BOOT] shell-first runtime ready\n'
        '[SMP] acknowledged TLB shootdowns: PASSED\n')
DESKTOP = ('[BOOT] desktop-first runtime ready\n'
           '[APPS] 10 signed applications installed\n'
           '[SESSION] WovenShell desktop started as pid 2\n'
           '[SESSION] diagnostic console\n'
           '[SMP] acknowledged TLB shootdowns: PASSED\n')


class NormalBootSerialSafetyTests(unittest.TestCase):
    def test_shell_success(self):
        self.assertEqual(SHELL.normal_boot_serial_errors(BASE, False), [])

    def test_desktop_success(self):
        self.assertEqual(SHELL.normal_boot_serial_errors(DESKTOP, True), [])

    def test_fatal_evidence_after_positive_markers(self):
        for marker in ('KERNEL PANIC', 'panicked at', 'SECURITY VIOLATION', 'security: failed'):
            with self.subTest(marker=marker):
                self.assertTrue(SHELL.normal_boot_serial_errors(DESKTOP + marker, True))

    def test_contradictory_failure_after_positive_markers(self):
        for marker in ('[BOOT] FAILED', '[SESSION] desktop FAIL',
                       '[APPS] installation FAILED', '[SMP] shootdown FAILED'):
            with self.subTest(marker=marker):
                self.assertTrue(SHELL.normal_boot_serial_errors(DESKTOP + marker + '\\n', True))

    def test_missing_desktop_and_diagnostic_evidence(self):
        self.assertTrue(SHELL.normal_boot_serial_errors(BASE, True))
        self.assertTrue(SHELL.normal_boot_serial_errors(
            DESKTOP.replace('[SESSION] diagnostic console', ''), True))

    def test_missing_keyboard_smp_evidence(self):
        self.assertTrue(SHELL.normal_boot_serial_errors(
            BASE.replace('[SMP] acknowledged TLB shootdowns: PASSED', ''), False))


if __name__ == '__main__':
    unittest.main()
