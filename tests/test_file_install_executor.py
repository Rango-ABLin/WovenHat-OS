import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts" / "execute-file-install.py"
SPEC = importlib.util.spec_from_file_location("file_executor", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def make_plan(image: Path) -> Path:
    digest = hashlib.sha256(image.read_bytes()).hexdigest()
    plan = {
        "valid": True,
        "plan": {
            "schema": "woveninstaller-plan-v1",
            "operations": {"disk_io": "none"},
            "target_disk": {"path": f"file-image:{digest}", "serial": "FILE-TEST-001"},
            "summary": {"target_bytes": image.stat().st_size},
            "filesystem": "wovenfs",
            "partitions": [{"name": "root", "size_bytes": 16 * 1024 * 1024}],
            "settings": {"user": {"username": "banda"}, "timezone": "UTC", "keyboard": "us", "network": {"mode": "disabled"}},
        },
    }
    path = image.parent / "validated-plan.json"
    path.write_text(json.dumps(plan), encoding="utf-8")
    return path


class FileInstallExecutorTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.image = self.root / "disposable.img"
        with self.image.open("wb") as stream:
            stream.truncate(20 * 1024 * 1024)
        self.plan = make_plan(self.image)

    def tearDown(self):
        self.temp.cleanup()

    def test_success_is_deterministic_and_digest_verifiable(self):
        result = MODULE.execute(self.image, self.plan)
        self.assertTrue(result["success"])
        self.assertEqual(result["manifest"]["mode"], "disposable-file-test-only")
        self.assertEqual(result["image_sha256"], hashlib.sha256(self.image.read_bytes()).hexdigest())
        self.assertTrue(self.image.read_bytes().startswith(MODULE.MAGIC))
        first = self.image.read_bytes()
        self.image.write_bytes(bytes(20 * 1024 * 1024))
        self.plan = make_plan(self.image)
        second = MODULE.execute(self.image, self.plan)
        self.assertEqual(first, self.image.read_bytes())
        self.assertEqual(result["image_sha256"], second["image_sha256"])

    def test_symlink_and_protected_identity_are_refused(self):
        link = self.root / "link.img"
        link.symlink_to(self.image)
        with self.assertRaisesRegex(MODULE.ExecutionError, "regular"):
            MODULE.execute(link, self.plan)
        protected = json.loads(self.plan.read_text(encoding="utf-8"))
        protected["plan"]["target_disk"]["path"] = r"\\.\PhysicalDrive1"
        self.plan.write_text(json.dumps(protected), encoding="utf-8")
        with self.assertRaisesRegex(MODULE.ExecutionError, "protected"):
            MODULE.execute(self.image, self.plan)

    def test_before_replace_failure_rolls_back_without_mutation(self):
        original = self.image.read_bytes()
        with self.assertRaisesRegex(MODULE.ExecutionError, "before atomic"):
            MODULE.execute(self.image, self.plan, "before-replace")
        self.assertEqual(original, self.image.read_bytes())
        self.assertEqual([], list(self.root.glob(".*")))

    def test_after_replace_failure_restores_original_atomically(self):
        original = self.image.read_bytes()
        with self.assertRaisesRegex(MODULE.ExecutionError, "after atomic"):
            MODULE.execute(self.image, self.plan, "after-replace")
        self.assertEqual(original, self.image.read_bytes())
        self.assertEqual([], list(self.root.glob(".*")))

    def test_digest_binding_rejects_plan_for_another_image(self):
        other = self.root / "other.img"
        other.write_bytes(b"other" * 1024)
        with self.assertRaisesRegex(MODULE.ExecutionError, "does not match"):
            MODULE.execute(other, self.plan)


if __name__ == "__main__":
    unittest.main()
