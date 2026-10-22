"""Host regressions for Stage 20.15 fail-closed desktop serial verification."""
import importlib.util
from pathlib import Path
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "test-stage20-15-session.py"
SPEC = importlib.util.spec_from_file_location("stage20_desktop_session_gate", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class DesktopSerialSafetyTests(unittest.TestCase):
    def test_nominal_desktop_evidence_is_accepted(self):
        self.assertFalse(MODULE.serial_has_fatal_evidence(
            "[S20.15] desktop session: PASSED\\n[SESSION] back to the desktop\\n"))

    def test_kernel_panic_overrides_desktop_pass(self):
        self.assertTrue(MODULE.serial_has_fatal_evidence(
            "[S20.15] desktop session: PASSED\\nKERNEL PANIC: page fault\\n"))

    def test_rust_panic_overrides_desktop_pass(self):
        self.assertTrue(MODULE.serial_has_fatal_evidence("thread panicked at kernel/src/task.rs"))

    def test_security_failure_overrides_desktop_pass(self):
        for marker in ("SECURITY VIOLATION", "SECURITY: FAILED"):
            with self.subTest(marker=marker):
                self.assertTrue(MODULE.serial_has_fatal_evidence(marker))


if __name__ == "__main__":
    unittest.main()
