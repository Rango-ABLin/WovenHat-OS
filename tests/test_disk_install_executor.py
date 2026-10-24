import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts" / "execute-disk-install.py"
SPEC = importlib.util.spec_from_file_location("disk_install_executor", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)

IMAGE_BYTES = 64 * 1024 * 1024
PARTITION_BYTES = 48 * 1024 * 1024
SETTINGS = {"user": {"username": "banda"}, "timezone": "UTC", "keyboard": "us", "network": {"mode": "disabled"}}


def make_plan(image: Path) -> Path:
    digest = hashlib.sha256(image.read_bytes()).hexdigest()
    plan = {
        "valid": True,
        "plan": {
            "schema": "woveninstaller-plan-v1",
            "operations": {"disk_io": "none", "format": "not-requested", "partition": "not-requested", "installation": "not-implemented"},
            "target_disk": {"path": f"file-image:{digest}", "serial": "DISK-TEST-001"},
            "partition_table": "gpt",
            "summary": {"target_bytes": image.stat().st_size},
            "filesystem": "fat32",
            "partitions": [{"name": "root", "size_bytes": PARTITION_BYTES}],
            "settings": SETTINGS,
        },
    }
    path = image.parent / "validated-plan.json"
    path.write_text(json.dumps(plan), encoding="utf-8")
    return path


class DiskInstallExecutorTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.image = self.root / "disposable.img"
        with self.image.open("wb") as stream:
            stream.truncate(IMAGE_BYTES)
        self.plan = make_plan(self.image)

    def tearDown(self):
        self.temp.cleanup()

    def test_success_performs_real_partition_format_and_bootloader(self):
        result = MODULE.execute(self.image, self.plan)
        self.assertTrue(result["success"])
        self.assertTrue(result["verified"])
        self.assertFalse(result["manifest"]["mock_filesystem_replacement"])
        self.assertTrue(result["manifest"]["partition_preserving"])
        self.assertEqual(result["image_sha256"], hashlib.sha256(self.image.read_bytes()).hexdigest())
        data = self.image.read_bytes()
        self.assertEqual(data[510:512], b"\x55\xaa")
        self.assertEqual(data[512:520], b"EFI PART")

    def test_explicit_real_artifact_is_verified_through_the_transaction(self):
        artifact = self.root / "BOOTX64.EFI"
        artifact.write_bytes(b"MZ" + bytes(range(251)))
        result = MODULE.execute(self.image, self.plan, artifact=artifact)
        self.assertTrue(result["verified"])
        self.assertEqual(result["efi"]["artifact_bytes"], artifact.stat().st_size)
        self.assertEqual(result["efi"]["artifact_sha256"], hashlib.sha256(artifact.read_bytes()).hexdigest())

    def test_settings_are_embedded_and_read_back_from_the_real_volume(self):
        MODULE.execute(self.image, self.plan)
        settings = MODULE.filesystem.read_installed_settings(self.image, self.plan)
        self.assertEqual(settings, SETTINGS)

    def test_rejects_non_fat32_filesystem(self):
        raw = json.loads(self.plan.read_text(encoding="utf-8"))
        raw["plan"]["filesystem"] = "wovenfs"
        self.plan.write_text(json.dumps(raw), encoding="utf-8")
        with self.assertRaisesRegex(MODULE.InstallError, "fat32"):
            MODULE.execute(self.image, self.plan)

    def test_symlink_and_protected_identity_are_refused(self):
        link = self.root / "link.img"
        link.symlink_to(self.image)
        with self.assertRaisesRegex(MODULE.InstallError, "regular"):
            MODULE.execute(link, self.plan)
        protected = json.loads(self.plan.read_text(encoding="utf-8"))
        protected["plan"]["target_disk"]["path"] = r"\\.\PhysicalDrive1"
        self.plan.write_text(json.dumps(protected), encoding="utf-8")
        with self.assertRaisesRegex(MODULE.InstallError, "protected"):
            MODULE.execute(self.image, self.plan)

    def test_each_injected_failure_restores_the_pristine_original_image(self):
        original = self.image.read_bytes()
        for failure in ("after-layout", "after-format", "after-efi"):
            with self.assertRaisesRegex(MODULE.InstallError, "injected failure"):
                MODULE.execute(self.image, self.plan, inject_failure=failure)
            self.assertEqual(self.image.read_bytes(), original, f"{failure} did not fully restore the image")
            self.assertEqual([], list(self.root.glob(".*")))

    def test_digest_binding_rejects_plan_for_another_image(self):
        other = self.root / "other.img"
        with other.open("wb") as stream:
            stream.write(b"\x01")
            stream.truncate(IMAGE_BYTES)
        with self.assertRaisesRegex(MODULE.InstallError, "does not match"):
            MODULE.execute(other, self.plan)


if __name__ == "__main__":
    unittest.main()
