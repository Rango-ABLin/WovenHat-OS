import subprocess
import tempfile
import unittest
from pathlib import Path
from PIL import Image
from capture_hello_gui import capture


class CaptureBridgeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "scripts").mkdir()
        (self.root / "scripts" / "screenshot-hello-gui-qemu.py").write_text("# fixture")
        self.output = self.root / "evidence" / "guest.png"

    def test_successful_capture_is_validated(self):
        def runner(command, **kwargs):
            Image.new("RGB", (16, 12), "#204060").save(self.output)
            return subprocess.CompletedProcess(command, 0, "", "")
        result = capture(self.root, self.output, runner=runner)
        self.assertEqual(result["size"], [16, 12])

    def test_nonzero_exit_fails(self):
        def runner(command, **kwargs):
            return subprocess.CompletedProcess(command, 1, "", "guest failed")
        with self.assertRaisesRegex(RuntimeError,"guest failed"):
            capture(self.root, self.output, runner=runner)

    def test_missing_capture_fails(self):
        def runner(command, **kwargs):
            return subprocess.CompletedProcess(command, 0, "", "")
        with self.assertRaisesRegex(ValueError,"without a PNG"):
            capture(self.root,self.output,runner=runner)

    def test_no_harness_fails(self):
        (self.root / "scripts" / "screenshot-hello-gui-qemu.py").unlink()
        with self.assertRaisesRegex(ValueError,"not found"):
            capture(self.root,self.output)

    def test_non_png_fails(self):
        def runner(command, **kwargs):
            self.output.write_text("not a PNG")
            return subprocess.CompletedProcess(command,0,"","")
        with self.assertRaises(OSError):
            capture(self.root,self.output,runner=runner)


if __name__=="__main__":
    unittest.main()
