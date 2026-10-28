import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).parents[1]
LAYOUT_SCRIPT = ROOT / "scripts" / "build-file-image-layout.py"
FORMAT_SCRIPT = ROOT / "scripts" / "format-file-image-fs.py"
layout_spec = importlib.util.spec_from_file_location("file_image_layout", LAYOUT_SCRIPT)
layout = importlib.util.module_from_spec(layout_spec)
assert layout_spec.loader is not None
layout_spec.loader.exec_module(layout)
format_spec = importlib.util.spec_from_file_location("file_image_filesystem", FORMAT_SCRIPT)
fs = importlib.util.module_from_spec(format_spec)
assert format_spec.loader is not None
format_spec.loader.exec_module(fs)


def make_plan(image: Path) -> Path:
    digest = hashlib.sha256(b"filesystem-plan").hexdigest()
    plan = {"valid": True, "plan": {"schema": "woveninstaller-plan-v1", "partition_table": "gpt", "target_disk": {"path": f"file-image:{digest}", "serial": "TEST-FS-001"}, "summary": {"target_bytes": image.stat().st_size}, "filesystem": "fat32", "partitions": [{"name": "root", "size_bytes": 48 * 1024 * 1024}], "settings": {}}}
    path = image.parent / "validated-plan.json"
    path.write_text(json.dumps(plan), encoding="utf-8")
    return path


class FileImageFilesystemTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.image = self.root / "disposable.img"
        with self.image.open("wb") as stream:
            stream.truncate(64 * 1024 * 1024)
        self.plan = make_plan(self.image)
        layout.build(self.image, self.plan, harness_created=True)

    def tearDown(self):
        self.temp.cleanup()

    def test_formats_only_data_region_and_verifies_deterministically(self):
        before = self.image.read_bytes()
        result = fs.format_image(self.image, self.plan, harness_created=True)
        after = self.image.read_bytes()
        self.assertTrue(result["valid"])
        self.assertEqual(result["filesystem"], "fat32-compatible")
        self.assertEqual(result["label"], "WOVENHAT")
        self.assertEqual(before[: layout.FIRST_USABLE * layout.SECTOR], after[: layout.FIRST_USABLE * layout.SECTOR])
        self.assertEqual(fs.verify(self.image, self.plan)["image_sha256"], result["image_sha256"])

    def test_injected_failures_restore_the_data_region(self):
        original = self.image.read_bytes()
        for failure in ("before-write", "after-write", "after-verify"):
            with self.assertRaisesRegex(fs.FormatError, "injected failure"):
                fs.format_image(self.image, self.plan, harness_created=True, inject_failure=failure)
            self.assertEqual(self.image.read_bytes(), original)

    def test_refuses_symlink_and_protected_identity(self):
        link = self.root / "link.img"
        try:
            link.symlink_to(self.image)
        except OSError as exc:
            if getattr(exc, "winerror", None) == 1314:
                self.skipTest("symlink creation requires SeCreateSymbolicLinkPrivilege or Developer Mode")
            raise
        with self.assertRaisesRegex(fs.FormatError, "symlink"):
            fs.format_image(link, self.plan, harness_created=True)
        bad = json.loads(self.plan.read_text(encoding="utf-8"))
        bad["plan"]["target_disk"]["path"] = r"\\.\PhysicalDrive1"
        self.plan.write_text(json.dumps(bad), encoding="utf-8")
        with self.assertRaisesRegex(fs.FormatError, "protected"):
            fs.format_image(self.image, self.plan, harness_created=True)

    def test_rejects_tampered_boot_metadata(self):
        fs.format_image(self.image, self.plan, harness_created=True)
        raw = bytearray(self.image.read_bytes())
        _, _, region = fs._read_layout(self.image, self.plan)
        data_offset, _, _ = fs._region_bounds(self.image, region)
        raw[data_offset + 510] ^= 1
        self.image.write_bytes(raw)
        with self.assertRaisesRegex(fs.FormatError, "boot signature|layout"):
            fs.verify(self.image, self.plan)

    def test_requires_harness_authorization(self):
        with self.assertRaisesRegex(fs.FormatError, "harness"):
            fs.format_image(self.image, self.plan)

    def test_embeds_and_reads_back_settings_file(self):
        settings = {"user": {"username": "banda"}, "timezone": "UTC", "keyboard": "us", "network": {"mode": "disabled"}}
        result = fs.format_image(self.image, self.plan, harness_created=True, settings=settings)
        self.assertTrue(result["valid"])
        self.assertTrue(result["settings_embedded"])
        self.assertEqual(fs.read_installed_settings(self.image, self.plan), settings)
        self.assertEqual(fs.verify(self.image, self.plan, settings=settings)["image_sha256"], result["image_sha256"])

    def test_no_settings_leaves_root_directory_empty(self):
        fs.format_image(self.image, self.plan, harness_created=True)
        self.assertIsNone(fs.read_installed_settings(self.image, self.plan))

    def test_oversized_settings_are_rejected(self):
        with self.assertRaisesRegex(fs.FormatError, "bounded single-cluster"):
            fs.format_image(self.image, self.plan, harness_created=True, settings={"padding": "x" * 1024})


if __name__ == "__main__":
    unittest.main()
