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
    def test_selected_policy_cannot_pass_on_an_ordinary_default_boot(self):
        log = "[BOOT] ALL VALIDATIONS PASSED\n[SMP] online=1 expected=1"
        self.assertTrue(MODULE.outcome_errors("recovery", 33, log, 1))
        log += "\n[BOOT POLICY] accepted version=1 entry=1 root=1 timeout_ms=0 command_line_bytes=8"
        self.assertEqual(MODULE.outcome_errors("recovery", 33, log, 1), [])

    def test_post_exit_failure_is_terminal_and_cannot_reach_kernel(self):
        with patch.object(MODULE, "CASES", {"page-table": "PageTableFrameExhausted"}):
            log = "TRACE: exiting boot services\n[WBOOT] post-exit resource failure: PageTableFrameExhausted\n[WBOOT] boot stopped: post-exit resource failure"
            self.assertEqual(MODULE.outcome_errors("page-table", None, log, 1), [])
            self.assertTrue(MODULE.outcome_errors("page-table", 33, log, 1))
            self.assertTrue(MODULE.outcome_errors("page-table", None, log + "\n[BOOT]", 1))

    def test_kernel_failure_requires_its_own_markers(self):
        log = "[WBOOT] invalid kernel image: Empty\n[WBOOT] boot stopped: invalid kernel image"
        self.assertEqual(MODULE.outcome_errors("kernel-empty", None, log, 1), [])
        self.assertTrue(MODULE.outcome_errors("kernel-missing", None, log, 1))
        self.assertTrue(MODULE.outcome_errors("kernel-empty", 33, log, 1))
        self.assertTrue(MODULE.outcome_errors("kernel-empty", None, log + "\n[BOOT]", 1))
        self.assertTrue(MODULE.outcome_errors("kernel-empty", None, log.replace("kernel image", "boot configuration"), 1))

    def test_loader_resource_failure_requires_its_own_markers(self):
        log = "[WBOOT] firmware resource failure: LoaderAllocation\n[WBOOT] boot stopped: firmware resource failure"
        self.assertEqual(MODULE.outcome_errors("kernel-allocation", None, log, 1), [])
        self.assertTrue(MODULE.outcome_errors("kernel-allocation", 33, log, 1))
        self.assertTrue(MODULE.outcome_errors("kernel-allocation", None, log + "\n[BOOT]", 1))
        self.assertTrue(MODULE.outcome_errors("kernel-allocation", None, log.replace("firmware resource", "kernel image"), 1))

    def test_disk_resource_failures_require_exact_firmware_markers(self):
        log = "[WBOOT] firmware resource failure: DiskRead\n[WBOOT] boot stopped: firmware resource failure"
        self.assertEqual(MODULE.outcome_errors("disk-read", None, log, 1), [])
        self.assertTrue(MODULE.outcome_errors("disk-directory", None, log, 1))
        self.assertTrue(MODULE.outcome_errors("disk-read", 33, log, 1))
        self.assertTrue(MODULE.outcome_errors("disk-read", None, log + "\n[BOOT]", 1))

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
