"""Host regression tests for the GFX-07 fail-closed output detector."""
import importlib.util
import pathlib
import unittest
from tempfile import TemporaryDirectory
from unittest import mock
from contextlib import ExitStack
import subprocess
import sys

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts" / "test-gfx07-qemu-matrix.py"
spec = importlib.util.spec_from_file_location("gfx07_matrix", SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class FatalMarkers(unittest.TestCase):
    def test_rejects_failed_and_timeout_markers(self):
        for line in (
            "[S20.14B] desktop across monitors: FAILED",
            "[S20.15] session TIMEOUT",
            "kernel panic: unexpected state",
            "panicked at kernel/src/display.rs",
            "security violation",
        ):
            with self.subTest(line=line):
                self.assertIsNotNone(module.FATAL.search(line))

    def test_allows_success_and_incidental_words(self):
        for line in (
            "[S20.14B] desktop across monitors: PASSED",
            "graphics fallback activated safely",
            "failure-free rendering path",
            "timeout_ns setting is 900",
            "staged frame presented",
        ):
            with self.subTest(line=line):
                self.assertIsNone(module.FATAL.search(line))


    def test_source_is_checked_before_evidence_directory_created(self):
        with TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            qemu, firmware = root / "qemu", root / "OVMF.fd"
            qemu.write_bytes(b"mock")
            firmware.write_bytes(b"mock")
            output = root / "audit-artifacts" / "gfx07"
            observed = []

            def fake_run(command, **kwargs):
                if command[:3] == ["git", "rev-parse", "HEAD"]:
                    return subprocess.CompletedProcess(command, 0, stdout="f" * 40 + "\n", stderr="")
                if command[:3] == ["git", "status", "--porcelain"]:
                    observed.append(output.exists())
                    return subprocess.CompletedProcess(command, 0, stdout="", stderr="")
                return subprocess.CompletedProcess(command, 0, stdout=b"PASS\n", stderr=b"")

            argv = ["gfx07", "--qemu", str(qemu), "--firmware", str(firmware),
                    "--output", str(output)]
            with ExitStack() as stack:
                stack.enter_context(mock.patch.object(module, "ROOT", root))
                stack.enter_context(mock.patch.object(module.subprocess, "run", side_effect=fake_run))
                stack.enter_context(mock.patch.object(sys, "argv", argv))
                self.assertEqual(module.main(), 0)
            self.assertEqual(observed, [False])
            self.assertTrue((output / "manifest.json").is_file())


if __name__ == "__main__":
    unittest.main()
