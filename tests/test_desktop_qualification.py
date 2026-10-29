"""Host-only contract tests for the real-QEMU desktop qualification gate."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "test-desktop-qualification.py"
SPEC = importlib.util.spec_from_file_location("desktop_qualification", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class DesktopQualificationTests(unittest.TestCase):
    def ppm(self, pixels: bytes, header: bytes = b"P6\n4 2\n255\n") -> bytes:
        return header + pixels

    def test_ppm_requires_real_nonuniform_frame(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "frame.ppm"
            path.write_bytes(self.ppm(b"\x00\x00\x00" * 7 + b"\xff\x00\x00"))
            self.assertEqual(MODULE.parse_ppm(path)[:2], (4, 2))

    def test_ppm_rejects_uniform_and_truncated_frames(self):
        with tempfile.TemporaryDirectory() as directory:
            uniform = Path(directory) / "uniform.ppm"
            uniform.write_bytes(self.ppm(b"\x00\x00\x00" * 8))
            with self.assertRaisesRegex(ValueError, "uniform"):
                MODULE.parse_ppm(uniform)
            truncated = Path(directory) / "truncated.ppm"
            truncated.write_bytes(self.ppm(b"\x00\x00\x00" * 7))
            with self.assertRaisesRegex(ValueError, "payload length"):
                MODULE.parse_ppm(truncated)

    def test_changed_pixels_counts_rgb_pixels(self):
        left = b"\x00\x00\x00\xff\xff\xff"
        right = b"\x00\x00\x00\xff\x00\xff"
        self.assertEqual(MODULE.changed_pixels(left, right), 1)

    def test_refresh_oracle_rejects_stale_blank_and_incomplete_frames(self):
        one = (4, 12, 4, 4, 4, 4, 14)
        ink, paper = b"\x64\xdc\xb4", b"\x2d\x41\x5a"
        blank = paper * (10 * 14)
        self.assertFalse(MODULE.bitmap_matches(10, 14, blank, 0, 0, one, ink, paper))
        self.assertFalse(MODULE.bitmap_matches(10, 14, blank[:-1], 0, 0, one, ink, paper))
        self.assertFalse(MODULE.bitmap_matches(10, 14, blank, 1, 0, one, ink, paper))

    def test_missing_qemu_and_firmware_are_unsupported(self):
        blockers = MODULE.preflight(Path("Z:/missing-qemu.exe"), Path("Z:/missing-ovmf.fd"))
        self.assertEqual(len(blockers), 2)
        self.assertTrue(all("unavailable" in blocker for blocker in blockers))

    def test_unsupported_result_never_claims_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            result = {
                "status": "UNSUPPORTED",
                "pass": False,
                "unsupported_blockers": ["QEMU unavailable"],
            }
            MODULE.write_result(output, result)
            decoded = json.loads((output / "result.json").read_text(encoding="utf-8"))
            self.assertEqual(decoded["status"], "UNSUPPORTED")
            self.assertFalse(decoded["pass"])

    def test_fatal_serial_evidence_overrides_any_guest_marker(self):
        self.assertFalse(MODULE.serial_has_fatal_evidence("woven-shell: desktop ready\n"))
        self.assertTrue(MODULE.serial_has_fatal_evidence(
            "woven-shell: desktop ready\nKERNEL PANIC: page fault\n"
        ))

    def test_default_firmware_is_adjacent_to_qemu(self):
        qemu = Path("C:/qemu/qemu-system-x86_64.exe")
        self.assertEqual(
            MODULE.resolve_firmware(qemu, None),
            Path("C:/qemu/share/edk2-x86_64-code.fd"),
        )


if __name__ == "__main__":
    unittest.main()
