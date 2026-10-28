import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "hardware-qualification-stimulator.py"


def load_module():
    spec = importlib.util.spec_from_file_location("hardware_qualification_stimulator", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load stimulator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class HardwareQualificationStimulatorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.stimulator = load_module()

    def invoke(self, *args):
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )

    def test_deterministic_replay(self):
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            args = (
                "--seed", "0x51A7E", "--cpus", "4", "--ram-mib", "4096",
                "--profile", "network.link-and-packet-faults", "--fault", "drop",
            )
            self.assertEqual(self.invoke(*args, "--output-dir", first).returncode, 0)
            self.assertEqual(self.invoke(*args, "--output-dir", second).returncode, 0)
            left = json.loads((Path(first) / "result.json").read_text(encoding="utf-8"))
            right = json.loads((Path(second) / "result.json").read_text(encoding="utf-8"))
            left["profiles"][0].pop("evidence_file")
            right["profiles"][0].pop("evidence_file")
            self.assertEqual(left, right)

    def test_invalid_values_fail(self):
        cases = (
            ("--profile", "not-a-profile"),
            ("--fault", "not-a-fault"),
            ("--cpus", "3"),
            ("--ram-mib", "512"),
            ("--physical-device", "\\\\.\\PhysicalDrive0"),
        )
        for case in cases:
            with self.subTest(case=case):
                result = self.invoke(*case)
                self.assertNotEqual(result.returncode, 0)

    def test_evidence_output_contains_required_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.invoke(
                "--seed", "7", "--cpus", "1", "--ram-mib", "256",
                "--profile", "input.ps2_keyboard", "--output-dir", directory,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            root = Path(directory)
            manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
            output = json.loads((root / "result.json").read_text(encoding="utf-8"))
            evidence = root / "evidence" / "input.ps2_keyboard.json"
            self.assertTrue(evidence.is_file())
            self.assertEqual(manifest["physical_status"], "PHYSICAL_NOT_TESTED")
            self.assertEqual(output["profiles"][0]["status"], "EMULATED_PASS")
            self.assertEqual(output["profiles"][0]["cpu_count"], 1)
            self.assertEqual(output["profiles"][0]["ram_mib"], 256)
            self.assertTrue(output["profiles"][0]["deterministic_event_sequence"])

    def test_explicit_no_physical_claim(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.invoke("--profile", "wifi.protocol-fixtures", "--output-dir", directory)
            self.assertEqual(result.returncode, 0, result.stderr)
            output = json.loads((Path(directory) / "result.json").read_text(encoding="utf-8"))
            self.assertEqual(output["profiles"][0]["status"], "SOFTWARE_ONLY_PASS")
            self.assertEqual(output["profiles"][0]["physical_status"], "PHYSICAL_NOT_TESTED")
            self.assertFalse(output["safety"]["physical_device_access"])
            self.assertFalse(output["safety"]["native_hardware_claim"])
            self.assertFalse(output["safety"]["tier1_claim"])


if __name__ == "__main__":
    unittest.main()
