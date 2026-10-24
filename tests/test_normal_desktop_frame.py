"""Normal WovenShell QEMU screenshot integrity regression tests."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts' / 'test-shell-qemu.py'
SPEC = importlib.util.spec_from_file_location('normal_shell_qemu', SCRIPT)
SHELL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SHELL)


class DesktopFrameTests(unittest.TestCase):
    def check(self, data):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'frame.ppm'
            path.write_bytes(data)
            return SHELL.validate_desktop_frame(path)

    def frame(self, pixels, header=b'P6\n64 64\n255\n'):
        return header + pixels

    def test_nonuniform_frame_accepted(self):
        pixels = bytearray(b'\x00\x00\x00' * (64 * 64))
        pixels[12:15] = b'\x80\x90\xa0'
        self.assertEqual(self.check(self.frame(pixels)), (64, 64))

    def test_comment_header_accepted(self):
        pixels = b'\x00\x00\x00' * (64 * 64 - 1) + b'\xff\xff\xff'
        self.assertEqual(self.check(self.frame(pixels, b'P6\n# QEMU capture\n64 64\n255\n')), (64, 64))

    def test_uniform_black_or_solid_color_rejected(self):
        for color in (b'\x00\x00\x00', b'\x00\x30\x90'):
            with self.subTest(color=color), self.assertRaisesRegex(ValueError, 'uniform'):
                self.check(self.frame(color * (64 * 64)))

    def test_truncated_frame_rejected(self):
        with self.assertRaisesRegex(ValueError, 'Truncated'):
            self.check(self.frame(b'\x01\x02\x03' * (64 * 64 - 1)))

    def test_extra_frame_bytes_rejected(self):
        with self.assertRaisesRegex(ValueError, 'oversized'):
            self.check(self.frame(b'\x01\x02\x03' * (64 * 64 + 1)))

    def test_bad_format_and_dimensions_rejected(self):
        for header in (b'P3\n64 64\n255\n', b'P6\n0 64\n255\n',
                       b'P6\n64 64\n65535\n', b'P6\n64 64\n255'):
            with self.subTest(header=header), self.assertRaises(ValueError):
                self.check(self.frame(b'\x01\x02\x03' * (64 * 64), header))


if __name__ == '__main__':
    unittest.main()
