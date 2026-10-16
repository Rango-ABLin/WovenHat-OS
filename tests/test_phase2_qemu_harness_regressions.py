"""Regression guards for OVMF VARS isolation and Stage 13.16 QMP shutdown."""
import ast
import runpy
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class HarnessRegression(unittest.TestCase):
    def test_ring3_harnesses_isolate_vars(self):
        for name in ("monitor", "settings"):
            with self.subTest(harness=name):
                source = (ROOT / "scripts" / f"test-{name}-qemu.py").read_text()
                self.assertIn("'--firmware-vars'", source)
                self.assertIn("shutil.copyfile(firmware_vars, vars_copy)", source)
                self.assertIn("if=pflash,unit=0,format=raw,readonly=on", source)
                self.assertIn("if=pflash,unit=1,format=raw,file=", source)
                self.assertIn("code != 33", source)
                self.assertIn("[S16.1] out-of-kernel application: PASSED", source)

    def test_hid_qmp_disconnect_never_bypasses_acceptance(self):
        source = (ROOT / "scripts" / "test-stage13-16-hid-reports.py").read_text()
        ast.parse(source)
        self.assertIn("raise EOFError", source)
        self.assertIn("except (OSError, EOFError):", source)
        self.assertIn("process.wait(timeout=2)", source)
        self.assertIn("process.returncode != 33 or PASS_MARKER not in completed_log", source)
        self.assertIn("if process.returncode != 33:", source)
        self.assertIn("if PASS_MARKER not in log:", source)

    def test_qmp_protocol_responses_and_disconnects(self):
        # Exercise the actual QMP parser without starting QEMU or opening a socket.
        module = runpy.run_path(str(ROOT / "scripts" / "test-stage13-16-hid-reports.py"))
        qmp_type = module["Qmp"]
        cases = (
            ('{"return": {}}\\n', None),
            ('{"event": "STOP"}\\n{"return": {}}\\n', None),
            ('{"error": {"class": "GenericError", "desc": "rejected"}}\\n', RuntimeError),
            ('', EOFError),
            ('not-json\\n', ValueError),
        )
        class FakeQmpFile:
            def __init__(self, responses):
                self.responses = iter(responses.splitlines(keepends=True))
                self.writes = []

            def write(self, message):
                self.writes.append(message)

            def flush(self):
                pass

            def readline(self):
                return next(self.responses, '')

        for response, expected_error in cases:
            with self.subTest(response=response):
                qmp = qmp_type.__new__(qmp_type)
                qmp.file = FakeQmpFile(response)
                if expected_error:
                    with self.assertRaises(expected_error):
                        qmp.command("input-send-event", events=[])
                else:
                    self.assertEqual(qmp.command("input-send-event", events=[]), {})
                self.assertIn('"execute": "input-send-event"', ''.join(qmp.file.writes))


if __name__ == "__main__":
    unittest.main()
