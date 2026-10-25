import tempfile
import unittest
from pathlib import Path
from PIL import Image
from compare_frames import compare


class VisualRegressionTests(unittest.TestCase):
    def test_identical_guest_frame(self):
        with tempfile.TemporaryDirectory() as directory:
            frame = Path(directory) / 'frame.png'
            Image.new('RGBA', (80, 50), (24, 38, 60, 255)).save(frame)
            outcome = compare(frame, frame)
            self.assertTrue(outcome['pass'])
            self.assertEqual(outcome['max_absolute_channel_difference'], 0)

    def test_color_difference_reports_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            reference = Path(directory) / 'reference.png'
            actual = Path(directory) / 'actual.png'
            Image.new('RGBA', (32, 24), (10, 20, 30, 255)).save(reference)
            Image.new('RGBA', (32, 24), (10, 20, 40, 255)).save(actual)
            outcome = compare(reference, actual)
            self.assertFalse(outcome['pass'])
            self.assertEqual(outcome['max_absolute_channel_difference'], 10)

    def test_diff_output_is_real_rgba_image(self):
        with tempfile.TemporaryDirectory() as directory:
            reference = Path(directory) / 'reference.png'
            actual = Path(directory) / 'actual.png'
            output = Path(directory) / 'diff.png'
            Image.new('RGBA', (12, 9), (0, 0, 0, 255)).save(reference)
            Image.new('RGBA', (12, 9), (255, 0, 0, 255)).save(actual)
            compare(reference, actual, out=output)
            with Image.open(output) as diff:
                self.assertEqual(diff.size, (12, 9))
                self.assertEqual(diff.mode, 'RGBA')
                self.assertEqual(diff.getpixel((4, 4)), (255, 0, 0, 0))

    def test_resolution_mismatch_fails_without_diff_image(self):
        with tempfile.TemporaryDirectory() as directory:
            reference = Path(directory) / 'reference.png'
            actual = Path(directory) / 'actual.png'
            output = Path(directory) / 'diff.png'
            Image.new('RGB', (32, 24)).save(reference)
            Image.new('RGB', (31, 24)).save(actual)
            outcome = compare(reference, actual, out=output)
            self.assertEqual(outcome['reason'], 'dimension_mismatch')
            self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
