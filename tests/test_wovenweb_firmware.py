"""Writable OVMF state must be private to each run."""
import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
spec = importlib.util.spec_from_file_location("web_firmware", Path(__file__).resolve().parents[1]/"scripts/test-wovenweb-network-qemu.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
class FirmwareTests(unittest.TestCase):
    def test_vars_template_never_becomes_a_writable_guest_drive(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder); code = root/"code.fd"; template=root/"vars.fd"; code.write_bytes(b"code");template.write_bytes(b"original")
            outputs=[root/"one", root/"two"]
            for output in outputs:
                output.mkdir(); drives=module.firmware_drives(code,template,output)
                self.assertIn(f"if=pflash,format=raw,readonly=on,file={code}",drives)
                self.assertIn(f"if=pflash,unit=1,format=raw,file={output/'OVMF_VARS.fd'}",drives)
                self.assertNotIn(f"if=pflash,unit=1,format=raw,file={template}",drives)
            (outputs[0]/"OVMF_VARS.fd").write_bytes(b"guest changed")
            self.assertEqual(template.read_bytes(),b"original")
            self.assertEqual((outputs[1]/"OVMF_VARS.fd").read_bytes(),b"original")
    def test_preflight_refuses_missing_code_or_vars_before_build(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);qemu=root/"qemu";qemu.write_bytes(b"not executed")
            args=SimpleNamespace(qemu=str(qemu),firmware=root/"missing",firmware_vars=None)
            with self.assertRaises(FileNotFoundError):module.resolve_firmware(args)
            args.firmware=root/"code";args.firmware.write_bytes(b"code");args.firmware_vars=root/"missing-vars"
            with self.assertRaises(FileNotFoundError):module.resolve_firmware(args)
if __name__ == "__main__":unittest.main()
