import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts" / "build-file-image-layout.py"
SPEC = importlib.util.spec_from_file_location("file_image_layout", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def make_plan(image: Path) -> Path:
    digest = hashlib.sha256(b"plan-target").hexdigest()
    plan = {"valid": True, "plan": {"schema": "woveninstaller-plan-v1", "partition_table": "gpt", "target_disk": {"path": f"file-image:{digest}", "serial": "TEST-LAYOUT-001"}, "summary": {"target_bytes": image.stat().st_size}, "filesystem": "wovenfs", "partitions": [{"name": "root", "size_bytes": 16 * 1024 * 1024}], "settings": {}}}
    path = image.parent / "validated-plan.json"
    path.write_text(json.dumps(plan), encoding="utf-8")
    return path


class FileImageLayoutTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.image = self.root / "disposable.img"
        with self.image.open("wb") as stream:
            stream.truncate(64 * 1024 * 1024)
        self.plan = make_plan(self.image)

    def tearDown(self):
        self.temp.cleanup()

    def test_build_is_deterministic_and_verifies_regions(self):
        first = MODULE.build(self.image, self.plan, harness_created=True)
        image = self.image.read_bytes()
        second = MODULE.verify(self.image, self.plan)
        self.assertTrue(first["valid"])
        self.assertEqual(first["image_sha256"], second["image_sha256"])
        self.assertEqual(second["partition_count"], 2)
        self.assertEqual(image[510:512], b"\x55\xaa")
        self.assertEqual(image[450], 0xEE)
        self.assertEqual(image[512:520], b"EFI PART")

    def test_requires_harness_and_refuses_symlink_or_device_identity(self):
        with self.assertRaisesRegex(MODULE.LayoutError, "harness"):
            MODULE.build(self.image, self.plan)
        link = self.root / "link.img"
        try:
            link.symlink_to(self.image)
        except OSError as exc:
            if getattr(exc, "winerror", None) == 1314:
                self.skipTest("symlink creation requires SeCreateSymbolicLinkPrivilege or Developer Mode")
            raise
        with self.assertRaisesRegex(MODULE.LayoutError, "symlink"):
            MODULE.build(link, self.plan, harness_created=True)
        bad = json.loads(self.plan.read_text(encoding="utf-8"))
        bad["plan"]["target_disk"]["path"] = r"\\.\PhysicalDrive1"
        self.plan.write_text(json.dumps(bad), encoding="utf-8")
        with self.assertRaisesRegex(MODULE.LayoutError, "protected"):
            MODULE.build(self.image, self.plan, harness_created=True)

    def test_verify_detects_checksum_and_overlap_tampering(self):
        MODULE.build(self.image, self.plan, harness_created=True)
        raw = bytearray(self.image.read_bytes())
        offset = MODULE.FIRST_USABLE * MODULE.SECTOR + len(MODULE.METADATA_MAGIC) + 4
        offset = raw.find(b"disposable", offset)
        self.assertGreater(offset, 0)
        raw[offset] = ord("x")
        self.image.write_bytes(raw)
        with self.assertRaisesRegex(MODULE.LayoutError, "checksum"):
            MODULE.verify(self.image, self.plan)

        MODULE.build(self.image, self.plan, harness_created=True)
        raw = bytearray(self.image.read_bytes())
        start = MODULE.FIRST_USABLE * MODULE.SECTOR
        length_start = start + len(MODULE.METADATA_MAGIC)
        length = int.from_bytes(raw[length_start:length_start + 4], "little")
        payload_start = length_start + 4
        metadata = json.loads(bytes(raw[payload_start:payload_start + length]).decode())
        metadata["partitions"][1]["first_lba"] = metadata["partitions"][0]["first_lba"]
        unsigned = dict(metadata)
        unsigned.pop("sha256", None)
        metadata["sha256"] = hashlib.sha256(MODULE._canonical(unsigned)).hexdigest()
        encoded = MODULE._canonical(metadata)
        raw[payload_start:payload_start + len(encoded)] = encoded
        self.image.write_bytes(raw)
        with self.assertRaisesRegex(MODULE.LayoutError, "overlap"):
            MODULE.verify(self.image, self.plan)


if __name__ == "__main__":
    unittest.main()
