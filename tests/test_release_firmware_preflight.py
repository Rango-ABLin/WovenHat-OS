import importlib.util
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock


ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts" / "test-release.py"
SPEC = importlib.util.spec_from_file_location("test_release", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ReleaseFirmwarePreflightTests(unittest.TestCase):
    def test_explicit_qemu_and_firmware_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            qemu = Path(directory) / "qemu"
            firmware = Path(directory) / "OVMF_CODE.fd"
            qemu.write_bytes(b"")
            firmware.write_bytes(b"")
            args = SimpleNamespace(qemu=str(qemu), firmware=str(firmware))
            self.assertEqual(MODULE.resolve_fault_gate_firmware(args), (str(qemu), str(firmware)))

    def test_missing_firmware_fails_before_acceptance_chain(self):
        args = SimpleNamespace(qemu="C:/missing/qemu-system-x86_64.exe", firmware=None)
        with self.assertRaises(SystemExit) as raised:
            MODULE.resolve_fault_gate_firmware(args)
        self.assertIn("requires an OVMF CODE firmware file", str(raised.exception))

    def test_missing_qemu_fails_closed(self):
        args = SimpleNamespace(qemu=None, firmware=None)
        with mock.patch.object(MODULE.shutil, "which", return_value=None):
            with self.assertRaises(SystemExit) as raised:
                MODULE.resolve_fault_gate_firmware(args)
        self.assertIn("requires QEMU", str(raised.exception))


if __name__ == "__main__":
    unittest.main()
