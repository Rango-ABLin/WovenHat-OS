"""Foundation release gates must not certify contradictory serial evidence."""
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
spec = importlib.util.spec_from_file_location('qemu_validation', SCRIPTS / 'qemu_validation.py')
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


class FoundationValidationTests(unittest.TestCase):
    required = ('[SMP] online=4 expected=4', '[BOOT] ALL VALIDATIONS PASSED')

    def validate(self, suffix='', code=33):
        return validator.validation_errors(code, '\n'.join(self.required) + suffix, self.required)

    def test_complete_evidence_and_detailed_success(self):
        self.assertFalse(self.validate())
        log = '\n'.join(marker + ' detail=42' for marker in self.required)
        self.assertFalse(validator.validation_errors(33, log, self.required))

    def test_success_does_not_override_failure_before_or_after(self):
        for failure in ('[SMP] scheduler/barrier: FAILED', '[STORAGE] recovery: TIMEOUT',
                        'KERNEL PANIC', 'thread panicked at kernel/src/task.rs:1'):
            for log in (failure + '\n' + '\n'.join(self.required),
                        '\n'.join(self.required) + '\n' + failure):
                with self.subTest(log=log):
                    self.assertTrue(validator.validation_errors(33, log, self.required))

    def test_substrings_and_missing_markers_do_not_certify(self):
        for missing in self.required:
            log = '\n'.join(marker for marker in self.required if marker != missing)
            self.assertTrue(validator.validation_errors(33, log, self.required))
        for log in ('prefix ' + '\n'.join(self.required),
                    '\n'.join(self.required).replace('PASSED', 'PASSEDNESS'),
                    '\n'.join(self.required).replace('expected=4', 'expected=40')):
            self.assertTrue(validator.validation_errors(33, log, self.required))

    def test_wrong_exit_or_wrong_cpu_count_rejected(self):
        for code in (None, 0, 1, 35, -11):
            self.assertTrue(self.validate(code=code))
        self.assertTrue(validator.validation_errors(33, '[SMP] online=1 expected=1\n' + self.required[1], self.required))

    def test_deliberate_user_fault_is_not_a_kernel_failure(self):
        self.assertFalse(self.validate('\n[USER FAULT] page fault at guard; exit=-11'))

    def test_all_foundation_consumers_use_same_fail_closed_validator(self):
        with patch.dict(sys.modules, {'qemu_validation': validator}):
            for name in ('test-memory-qemu', 'test-storage-qemu', 'test-stage6-hotplug', 'test-network-qemu'):
                consumer_spec = importlib.util.spec_from_file_location(name, SCRIPTS / (name + '.py'))
                consumer = importlib.util.module_from_spec(consumer_spec)
                consumer_spec.loader.exec_module(consumer)
                self.assertIs(consumer.validation_errors, validator.validation_errors)
                self.assertTrue(consumer.validation_errors(33, '\n'.join(self.required) + '\n[SMP] gate: FAILED', self.required))


if __name__ == '__main__':
    unittest.main()
