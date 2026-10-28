"""Regression tests for IPC allocator probe evidence directory safety."""
import importlib.util
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "test-ipc-handle-generation.py"
SPEC = importlib.util.spec_from_file_location("ipc_probe", SCRIPT)
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class IpcProbeEvidenceDirectoryTests(unittest.TestCase):
    def test_creates_missing_parent_and_leaf(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = PROBE.create_evidence_directory(root, 123)
            self.assertEqual(output, root / "audit-artifacts" / "ipc-generation-probe-123")
            self.assertTrue(output.is_dir())

    def test_creates_leaf_when_parent_exists(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "audit-artifacts").mkdir()
            output = PROBE.create_evidence_directory(root, 456)
            self.assertTrue(output.is_dir())

    def test_rejects_collision_without_touching_existing_evidence(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = PROBE.create_evidence_directory(root, 789)
            marker = output / "manifest.json"
            marker.write_text("original", encoding="utf-8")
            with self.assertRaises(FileExistsError):
                PROBE.create_evidence_directory(root, 789)
            self.assertEqual(marker.read_text(encoding="utf-8"), "original")

    def test_parent_file_is_not_silently_replaced(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            parent = root / "audit-artifacts"
            parent.write_text("preserve", encoding="utf-8")
            with self.assertRaises((FileExistsError, NotADirectoryError)):
                PROBE.create_evidence_directory(root, 1000)
            self.assertEqual(parent.read_text(encoding="utf-8"), "preserve")


if __name__ == "__main__":
    unittest.main()
