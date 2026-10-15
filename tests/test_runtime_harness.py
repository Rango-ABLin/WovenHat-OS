"""Reject false Wi-Fi acceptance, even when QEMU exits successfully."""
import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    'runtime_harness',
    Path(__file__).resolve().parents[1] / 'scripts/test-stage10-runtime.py')
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)


class RuntimeAcceptanceTests(unittest.TestCase):
    def test_compiler_tls_requires_fresh_threads_and_original_thread_gate(self):
        required = harness.required_markers('11.2-tls', 4)
        for marker in ('[S11.TLS] static compiler TLS: PASSED',
                       '[S11.TLS] fresh child TLS: PASSED',
                       '[S11.TLS] relocated pointer initializer: PASSED',
                       '[S11.TLS] biased PIE initializer relocation: PASSED',
                       '[S11.TLS] compiled fork state and fresh descendant: PASSED',
                       '[S11.TLS] compiled capacity cancellation and stale joins: PASSED',
                       '[S11.TLS] compiled owner-exit sibling armed: PASSED',
                       '[S11.TLS] nonzero congruence TLS: PASSED',
                       '[S11.TLS] page-edge TCB: PASSED',
                       '[S11.TLS] template/TCB rollback and fork protection: PASSED',
                       '[S11.TLS] compiler TLS lifecycle: PASSED',
                       '[S11.2] threads/TLS/join: PASSED'):
            self.assertIn(marker, required)
        for cpu in range(4):
            self.assertIn(f'[S11.TLS] lifecycle CPU {cpu}: PASSED', required)
        for missing in required:
            with self.subTest(missing=missing):
                log = '\n'.join(marker for marker in required if marker != missing)
                self.assertTrue(harness.validation_errors(33, log, required))

    def test_compiler_gate_preserves_all_original_loader_checks(self):
        required = harness.required_markers('11.5-compiler', 4)
        for marker in harness.required_markers('11.5', 4):
            self.assertIn(marker, required)
        self.assertIn('[S11.COMPILER] signed toolchain PIE: PASSED', required)
        self.assertIn('[S11.COMPILER] actual shared import and exact owner teardown: PASSED', required)
        self.assertIn('[S11.COMPILER] nonzero-bias shared PIE and teardown: PASSED', required)
        self.assertFalse(harness.validation_errors(33, '\n'.join(required), required))
        for missing in required:
            with self.subTest(missing=missing):
                log = '\n'.join(marker for marker in required if marker != missing)
                self.assertTrue(harness.validation_errors(33, log, required))

    def test_exit_status_fixture_and_original_thread_gate_are_required(self):
        required = harness.required_markers('11.2-status', 4)
        self.assertIn('[S11.PLACEMENT] fixed ET_EXEC at nonzero ASLR candidate: PASSED', required)
        self.assertIn('[S11.PLACEMENT] PIE executes with nonzero ASLR: PASSED', required)
        self.assertIn('[S11.2] threads/TLS/join: PASSED', required)
        self.assertTrue(any('ownership/teardown' in marker for marker in required))
        for missing in required:
            with self.subTest(missing=missing):
                log = '\n'.join(marker for marker in required if marker != missing)
                self.assertTrue(harness.validation_errors(33, log, required))

    def test_shared_library_evidence_is_required(self):
        required = harness.required_markers('11.5', 4)
        self.assertTrue(any('Ring-3 import call and owner teardown' in marker for marker in required))
        self.assertTrue(any('tampered signature rejected' in marker for marker in required))
        self.assertTrue(any('argv failure rollback' in marker for marker in required))
        for missing in required:
            with self.subTest(missing=missing):
                log = '\n'.join(marker for marker in required if marker != missing)
                self.assertTrue(harness.validation_errors(33, log, required))

    def test_complete_wifi_evidence_passes(self):
        required = harness.required_markers('13.9', 4)
        self.assertFalse(harness.validation_errors(33, '\n'.join(required), required))

    def test_old_wifi_gate_cannot_certify_ac(self):
        old_log = ('[SMP] online=4 expected=4\n'
                   '[S10.3] userspace async completion ABI + cancellation/teardown: PASSED\n'
                   '[S13.9H] GTK + encrypted key data: PASSED')
        errors = harness.validation_errors(
            33, old_log, harness.required_markers('13.9', 4))
        self.assertTrue(any('[S13.10AC]' in error for error in errors))

    def test_every_wifi_prerequisite_is_required(self):
        required = harness.required_markers('13.9', 2)
        for missing in required:
            with self.subTest(missing=missing):
                log = '\n'.join(marker for marker in required if marker != missing)
                self.assertTrue(harness.validation_errors(33, log, required))

    def test_failure_or_panic_overrides_success_markers(self):
        required = harness.required_markers('13.9', 1)
        for failure in ('[S13.10AC] deferred service: FAILED', 'KERNEL PANIC',
                        '[S13.10AC] deferred service: TIMEOUT'):
            with self.subTest(failure=failure):
                log = '\n'.join([*required, failure])
                self.assertTrue(harness.validation_errors(33, log, required))

    def test_wrong_exit_or_cpu_count_fails(self):
        required = harness.required_markers('13.9', 4)
        self.assertTrue(harness.validation_errors(0, '\n'.join(required), required))
        self.assertTrue(harness.validation_errors(
            33, '\n'.join(harness.required_markers('13.9', 1)), required))


    def test_stage13_2_requires_topology_lifecycle_marker(self):
        required = harness.required_markers('13.2', 2)
        self.assertIn('[S13.2] PCIe topology + generation-safe ownership/teardown: PASSED', required)
        old_log = '\n'.join([
            '[SMP] online=2 expected=2',
            '[S10.3] userspace async completion ABI + cancellation/teardown: PASSED',
            '[S13.2] PCI/PCIe configuration + inventory: PASSED',
        ])
        self.assertTrue(harness.validation_errors(33, old_log, required))


if __name__ == '__main__':
    unittest.main()
