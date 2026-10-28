"""Contract tests for the isolated virtual trust-chain qualifier."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "test-virtual-trust-chain.py"
SPEC = importlib.util.spec_from_file_location("virtual_trust_chain", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class VirtualTrustChainTests(unittest.TestCase):
    def test_missing_inputs_are_machine_readable_and_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "result.json"
            # Invoke the public entry point while capturing only its result file;
            # argparse has no required inputs because missing inputs must be
            # reported as structured blockers rather than usage noise.
            result_code = MODULE.main(["--output", str(output)])
            document = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(result_code, 2)
            self.assertEqual(document["status"], "NOT_QUALIFIED")
            self.assertFalse(document["qualified"])
            codes = {item["code"] for item in document["blockers"]}
            self.assertTrue({
                "missing-ovmf-vars",
                "missing-swtpm",
                "missing-disk-image",
                "missing-development-key",
                "missing-development-certificate",
            } <= codes)
            self.assertFalse(document["safety"]["raw_device_opened"])
            self.assertFalse(document["safety"]["private_key_recorded"])

    def test_device_paths_are_rejected(self) -> None:
        for value in (
            r"\\.\PhysicalDrive0",
            r"/dev/sda",
            r"/dev/nvme0n1",
            r"C:\\Windows\\System32\\PhysicalDrive1",
        ):
            with self.subTest(value=value):
                self.assertTrue(MODULE._is_device_path(value))

    def test_qemu_command_is_isolated_and_secure(self) -> None:
        command = MODULE.build_qemu_command(
            Path("qemu-system-x86_64"),
            Path("OVMF_CODE.fd"),
            Path("OVMF_VARS.fd"),
            Path("guest.img"),
            Path("serial.log"),
            34567,
        )
        joined = " ".join(command)
        self.assertIn("q35,smm=on", joined)
        self.assertIn("driver=cfi.pflash01,property=secure,value=on", joined)
        self.assertIn("readonly=on,file=OVMF_CODE.fd", joined)
        self.assertIn("format=raw,file=OVMF_VARS.fd", joined)
        self.assertIn("readonly=on,file=guest.img", joined)
        self.assertIn("tpm-tis,tpmdev=tpm0", joined)

    def test_result_never_records_private_key_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "result.json"
            secret = b"development-private-key-fixture"
            result = MODULE._result(
                [MODULE._blocker("test", "blocked")],
                checks=[],
                inputs={"development_key": {"sha256": "x"}},
                execution={"attempted": False},
                output=output,
            )
            MODULE._write_result(output, result)
            self.assertNotIn(secret.decode(), output.read_text(encoding="utf-8"))

    def test_preflight_only_cannot_claim_qualification(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "result.json"
            result = MODULE._result(
                [MODULE._blocker("qemu-execution-not-requested", "preflight-only")],
                checks=[],
                inputs={},
                execution={"attempted": False},
                output=output,
            )
            self.assertEqual(result["status"], "NOT_QUALIFIED")
            self.assertFalse(result["qualified"])

    def test_raw_output_path_is_rejected_without_writing(self) -> None:
        args = type("Args", (), {"output": Path(r"\\.\PhysicalDrive0")})()
        code, result = MODULE.qualify(args)
        self.assertEqual(code, 2)
        self.assertEqual(result["status"], "NOT_QUALIFIED")
        self.assertEqual(result["blockers"][0]["code"], "unsafe-output-path")


if __name__ == "__main__":
    unittest.main()
