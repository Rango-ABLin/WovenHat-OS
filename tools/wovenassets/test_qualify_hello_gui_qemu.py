import tempfile
import subprocess
import unittest
from pathlib import Path
from qualify_hello_gui_qemu import run


class EndToEndHarnessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = self.root / "approved-baselines.json"
        self.manifest.write_text("{}")
        self.output = self.root / "out"

    def test_capture_then_qualify(self):
        calls = []
        def capture(root, output, qemu, firmware, timeout):
            calls.append("capture")
            output.write_bytes(b"fixture")
            return {"capture": str(output), "size": [12, 10]}
        def qualify(manifest, identifier, image):
            calls.append("qualify")
            self.assertTrue(image.is_file())
            return {"pass": True, "baseline": identifier}
        result = run(self.root, self.manifest, "hello-gui", self.output,
                     capturer=capture, qualifier=qualify)
        self.assertEqual(calls, ["capture", "qualify"])
        self.assertTrue(result["pass"])
        self.assertTrue((self.output / "visual-qualification.json").is_file())

    def test_capture_failure_prevents_qualification(self):
        def failed(*args):
            raise RuntimeError("QEMU returned nonzero")
        def unexpected(*args):
            self.fail("qualifier must not run")
        with self.assertRaisesRegex(RuntimeError, "QEMU"):
            run(self.root, self.manifest, "hello-gui", self.output,
                capturer=failed, qualifier=unexpected)
        self.assertFalse((self.output / "visual-qualification.json").exists())

    def test_mismatch_reports_failure(self):
        def capture(root, output, qemu, firmware, timeout):
            output.write_bytes(b"fixture")
            return {"capture": str(output)}
        def qualify(manifest, identifier, image):
            return {"pass": False, "max_absolute_channel_difference": 3}
        result = run(self.root, self.manifest, "hello-gui", self.output,
                     capturer=capture, qualifier=qualify)
        self.assertFalse(result["pass"])

    def test_prior_success_not_reused_on_failure(self):
        self.output.mkdir()
        (self.output / "hello-gui.png").write_bytes(b"old")
        (self.output / "visual-qualification.json").write_text('{"pass":true}')
        def failed(*args):
            raise RuntimeError("failed")
        with self.assertRaises(RuntimeError):
            run(self.root, self.manifest, "hello-gui", self.output, capturer=failed)
        self.assertFalse((self.output / "hello-gui.png").exists())
        self.assertFalse((self.output / "visual-qualification.json").exists())

    def test_non_boolean_result_is_rejected(self):
        def capture(root, output, qemu, firmware, timeout):
            output.write_bytes(b"fixture")
            return {"capture": str(output)}
        def invalid_result(manifest, identifier, image):
            return {"pass": "success"}
        with self.assertRaisesRegex(ValueError, "explicit boolean"):
            run(self.root, self.manifest, "hello-gui", self.output,
                capturer=capture, qualifier=invalid_result)
        self.assertFalse((self.output / "visual-qualification.json").exists())

    def test_timeout_does_not_leave_success_report(self):
        def timeout(*args):
            raise subprocess.TimeoutExpired(["qemu-system-x86_64"], 90)
        with self.assertRaises(subprocess.TimeoutExpired):
            run(self.root, self.manifest, "hello-gui", self.output, capturer=timeout)
        self.assertFalse((self.output / "visual-qualification.json").exists())


if __name__ == "__main__":
    unittest.main()
