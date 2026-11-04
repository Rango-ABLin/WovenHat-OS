"""Ensure boot-policy acceptance cannot pass on a continued/rejected boot."""

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("boot_config_gate", ROOT / "scripts/test-boot-config-qemu.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class BootConfigGateTests(unittest.TestCase):
    def test_positive_requires_exit_and_kernel_marker(self):
        log = "[BOOT] ALL VALIDATIONS PASSED\n[SMP] online=1 expected=1 LAPIC/IOAPIC enabled"
        self.assertEqual(MODULE.outcome_errors("default", 33, log, 1), [])
        self.assertTrue(MODULE.outcome_errors("default", 0, log, 1))
        self.assertTrue(MODULE.outcome_errors("default", 33, log, 2))
        self.assertTrue(MODULE.outcome_errors("default", 33, "", 1))

    def test_rejection_requires_exact_reason_and_controlled_stop(self):
        log = "[WBOOT] invalid boot configuration: Malformed\n" + MODULE.STOP
        self.assertEqual(MODULE.outcome_errors("malformed", None, log, 1), [])
        self.assertTrue(MODULE.outcome_errors("unsupported", None, log, 1))
        self.assertTrue(MODULE.outcome_errors("malformed", 33, log, 1))

    def test_continuing_after_rejection_always_fails(self):
        log = "[WBOOT] invalid boot configuration: Malformed\n" + MODULE.STOP
        for continuation in ("TRACE: exiting boot services", "[BOOT] ALL VALIDATIONS PASSED", "KERNEL PANIC"):
            self.assertTrue(MODULE.outcome_errors("malformed", None, log + "\n" + continuation, 1))

    def test_source_snapshot_detects_edits_without_crlf_noise(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "runtime.rs"
            with patch.object(MODULE, "ROOT", root), patch.object(MODULE, "BOOTLOADER_SOURCES", ("runtime.rs",)):
                source.write_bytes(b"fn boot() {}\r\n")
                original = MODULE.source_snapshot()
                source.write_bytes(b"fn boot() {}\n")
                self.assertEqual(MODULE.source_snapshot(), original)
                source.write_bytes(b"fn boot() { panic!(); }\n")
                self.assertNotEqual(MODULE.source_snapshot(), original)

    def test_non_file_and_unbounded_inputs_are_rejected_before_hashing(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            with self.assertRaises(ValueError):
                MODULE.regular_input(path, 8)
            file = path / "input"
            file.write_bytes(b"")
            with self.assertRaises(ValueError):
                MODULE.regular_input(file, 8)
            file.write_bytes(b"123456789")
            with self.assertRaises(ValueError):
                MODULE.regular_input(file, 8)


if __name__ == "__main__":
    unittest.main()
