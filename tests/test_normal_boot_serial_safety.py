"""Fail-closed normal-boot serial evidence tests."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts' / 'test-shell-qemu.py'
SPEC = importlib.util.spec_from_file_location('shell_qemu', SCRIPT)
SHELL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SHELL)

# Any catalog size works; the harness reads the real one from the packaged
# system-app manifest.
APPS = 11
BASE = ('[BOOT] shell-first runtime ready\n'
        '[SMP] acknowledged TLB shootdowns: PASSED\n')
DESKTOP = ('[BOOT] desktop-first runtime ready\n'
           f'{SHELL.apps_installed_marker(APPS)}\n'
           '[SESSION] WovenShell desktop started as pid 2\n'
           '[SESSION] diagnostic console\n'
           '[SESSION] back to the desktop\n'
           '[SMP] acknowledged TLB shootdowns: PASSED\n')


class NormalBootSerialSafetyTests(unittest.TestCase):
    def test_shell_success(self):
        self.assertEqual(SHELL.normal_boot_serial_errors(BASE, False), [])

    def test_desktop_success(self):
        self.assertEqual(SHELL.normal_boot_serial_errors(DESKTOP, True, APPS), [])

    def test_old_install_marker_cannot_claim_wallpaper_readback(self):
        old = DESKTOP.replace(
            SHELL.apps_installed_marker(APPS),
            f'[APPS] {APPS} signed applications installed')
        self.assertTrue(SHELL.normal_boot_serial_errors(old, True, APPS))

    def test_fatal_evidence_after_positive_markers(self):
        for marker in ('KERNEL PANIC', 'panicked at', 'SECURITY VIOLATION', 'security: failed'):
            with self.subTest(marker=marker):
                self.assertTrue(SHELL.normal_boot_serial_errors(DESKTOP + marker, True, APPS))

    def test_contradictory_failure_after_positive_markers(self):
        for marker in ('[BOOT] FAILED', '[SESSION] desktop FAIL',
                       '[APPS] installation FAILED', '[SMP] shootdown FAILED'):
            with self.subTest(marker=marker):
                self.assertTrue(SHELL.normal_boot_serial_errors(DESKTOP + marker + '\\n', True, APPS))

    def test_missing_desktop_and_diagnostic_evidence(self):
        self.assertTrue(SHELL.normal_boot_serial_errors(BASE, True, APPS))
        self.assertTrue(SHELL.normal_boot_serial_errors(
            DESKTOP.replace('[SESSION] diagnostic console', ''), True, APPS))

    def test_install_marker_follows_catalog_size(self):
        self.assertTrue(SHELL.normal_boot_serial_errors(DESKTOP, True, APPS + 1))
        with self.assertRaises(ValueError):
            SHELL.normal_boot_serial_errors(DESKTOP, True)

    def test_missing_keyboard_smp_evidence(self):
        self.assertTrue(SHELL.normal_boot_serial_errors(
            BASE.replace('[SMP] acknowledged TLB shootdowns: PASSED', ''), False))

    def test_phase_d_timing_requires_truthful_monotonic_stages(self):
        log = ('[BOOT PROFILE] KERNEL BOOT [1/7]: graphics surface ready timing=pre-timer\n'
               '[BOOT PROFILE] timer ready tick=0 resolution_ms=10 scope=kernel-post-timer\n')
        total = 0
        for stage in ('2/7', '3/7', '4/7', '5/7', '6/7', '7/7'):
            total += 3
            log += (f'[BOOT PROFILE] KERNEL BOOT [{stage}]: ready '
                    f'delta_ticks=3 total_ticks={total}\n')
        SHELL.validate_phase_d_timing(log)
        with self.assertRaises(ValueError):
            SHELL.validate_phase_d_timing(log.replace('total_ticks=9', 'total_ticks=99'))
        with self.assertRaises(ValueError):
            SHELL.validate_phase_d_timing(log.replace('timing=pre-timer',
                                                       'delta_ticks=0 total_ticks=0'))

    def test_phase_d_splash_palette_fails_closed(self):
        width = height = 64
        pixels = bytearray(bytes((4, 12, 28)) * (width * height))
        for index in range(120):
            pixels[index * 3:index * 3 + 3] = bytes((255, 205, 96))
            offset = (width * height - 1 - index) * 3
            pixels[offset:offset + 3] = bytes((40, 220, 230))
        with tempfile.TemporaryDirectory() as directory:
            frame = Path(directory) / 'splash.ppm'
            frame.write_bytes(f'P6\n{width} {height}\n255\n'.encode() + pixels)
            self.assertEqual(SHELL.validate_splash_frame(frame)[:2], (width, height))
            frame.write_bytes(f'P6\n{width} {height}\n255\n'.encode()
                              + bytes((4, 12, 28)) * (width * height))
            with self.assertRaises(ValueError):
                SHELL.validate_splash_frame(frame)


if __name__ == '__main__':
    unittest.main()
