import importlib.util
import types
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory


SCRIPT = Path(__file__).parents[1] / "scripts" / "test-stage31-randomized-faults.py"
SPEC = importlib.util.spec_from_file_location("stage31_randomized", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class Stage31RandomizedFaultTests(unittest.TestCase):
    def test_network_scenario_uses_seeded_multi_flap(self):
        args = types.SimpleNamespace(
            cpus=4,
            qemu="qemu-system-x86_64",
            firmware=Path("OVMF_CODE.fd"),
            seed=0x51A7E,
        )
        command = MODULE.scenario_command("network-loss", args)
        self.assertIn("--link-loss", command)
        self.assertEqual(command[command.index("--link-loss-count") + 1], "3")
        self.assertEqual(command[command.index("--fault-seed") + 1], "0x51a7e")

    def test_storage_read_fault_scenario_uses_disposable_qemu_gate(self):
        args = types.SimpleNamespace(
            cpus=16,
            qemu="qemu-system-x86_64",
            firmware=Path("OVMF_CODE.fd"),
            seed=0x51A7E,
        )
        command = MODULE.scenario_command("storage-read-fault", args)
        self.assertIn("test-stage31-storage-read-fault.py", command[1])
        self.assertEqual(command[command.index("--cpus") + 1], "16")
        self.assertIn("--firmware", command)

    def test_storage_write_fault_scenario_uses_disposable_qemu_gate(self):
        args = types.SimpleNamespace(
            cpus=1,
            qemu="qemu-system-x86_64",
            firmware=Path("OVMF_CODE.fd"),
            seed=0x51A7E,
        )
        command = MODULE.scenario_command("storage-write-fault", args)
        self.assertIn("test-stage31-storage-write-fault.py", command[1])
        self.assertIn("--firmware", command)

    def test_existing_evidence_gets_a_fresh_rerun_directory(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            base = root / "audit-artifacts" / "stage31-randomized-51a7e-1cpu-9scenarios"
            base.mkdir(parents=True)
            (base / "historical.result.json").write_text("{}", encoding="utf-8")
            evidence = MODULE.allocate_evidence(root, 0x51A7E, 1, 9)
            self.assertNotEqual(evidence, base)
            self.assertFalse(any(evidence.iterdir()))

    def test_empty_evidence_uses_canonical_directory(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = MODULE.allocate_evidence(root, 0x51A7E, 1, 9)
            self.assertEqual(
                evidence,
                root / "audit-artifacts" / "stage31-randomized-51a7e-1cpu-9scenarios",
            )
