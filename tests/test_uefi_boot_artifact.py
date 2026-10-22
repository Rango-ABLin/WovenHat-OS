import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).parents[1]
LAYOUT_SCRIPT = ROOT / "scripts" / "build-file-image-layout.py"
SCRIPT = ROOT / "scripts" / "prepare-uefi-boot-artifact.py"
layout_spec = importlib.util.spec_from_file_location("file_image_layout", LAYOUT_SCRIPT)
layout = importlib.util.module_from_spec(layout_spec)
assert layout_spec.loader is not None
layout_spec.loader.exec_module(layout)
artifact_spec = importlib.util.spec_from_file_location("uefi_boot_artifact", SCRIPT)
artifact = importlib.util.module_from_spec(artifact_spec)
assert artifact_spec.loader is not None
artifact_spec.loader.exec_module(artifact)


def make_plan(image: Path) -> Path:
    plan = {
        "valid": True,
        "plan": {
            "schema": "woveninstaller-plan-v1",
            "partition_table": "gpt",
            "target_disk": {"path": "file-image:uefi-test", "serial": "TEST-UEFI-001"},
            "summary": {"target_bytes": image.stat().st_size},
            "filesystem": "fat32",
            "partitions": [{"name": "root", "size_bytes": 48 * 1024 * 1024}],
            "settings": {},
        },
    }
    path = image.parent / "validated-plan.json"
    path.write_text(json.dumps(plan), encoding="utf-8")
    return path


class UefiBootArtifactTests(unittest.TestCase):
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

    def test_fixture_is_placed_at_exact_uefi_path_and_verified(self):
        result = artifact.prepare(self.image, self.plan, fixture=True, harness_created=True)
        self.assertTrue(result["valid"])
        self.assertEqual(result["path"], "EFI/BOOT/BOOTX64.EFI")
        self.assertEqual(result["artifact_sha256"], hashlib.sha256(artifact.FIXTURE_BYTES).hexdigest())
        self.assertEqual(artifact.verify(self.image, self.plan)["bootability"], "not-qualified")

    def test_explicit_artifact_digest_and_esp_bounds_are_verified(self):
        source = self.root / "bootx64.efi"
        payload = b"MZ" + bytes(range(256))
        source.write_bytes(payload)
        result = artifact.prepare(self.image, self.plan, artifact=source, harness_created=True)
        self.assertEqual(result["artifact_bytes"], len(payload))
        self.assertEqual(result["artifact_sha256"], hashlib.sha256(payload).hexdigest())
        _, _, region = artifact._read_layout(self.image, self.plan)
        offset, size = artifact._esp_bounds(self.image, region)
        self.assertLess(offset + artifact.PAYLOAD_ALIGNMENT + len(payload), offset + size)

    def test_injected_failures_restore_the_entire_esp_region(self):
        original = self.image.read_bytes()
        for failure in ("before-write", "after-write", "after-verify"):
            with self.assertRaisesRegex(artifact.ArtifactError, "injected failure"):
                artifact.prepare(self.image, self.plan, fixture=True, harness_created=True, inject_failure=failure)
            self.assertEqual(self.image.read_bytes(), original)

    def test_detects_path_checksum_and_payload_tampering(self):
        artifact.prepare(self.image, self.plan, fixture=True, harness_created=True)
        raw = bytearray(self.image.read_bytes())
        esp_offset = artifact._esp_bounds(self.image, artifact._read_layout(self.image, self.plan)[2])[0]
        metadata_start = esp_offset + len(artifact.HEADER_MAGIC) + 4
        length = int.from_bytes(raw[esp_offset + len(artifact.HEADER_MAGIC):metadata_start], "little")
        metadata = json.loads(bytes(raw[metadata_start:metadata_start + length]).decode())
        metadata["path"] = "EFI/BOOT/OTHER.EFI"
        encoded = artifact._canonical(metadata)
        raw[metadata_start:metadata_start + length] = encoded + b" " * (length - len(encoded))
        self.image.write_bytes(raw)
        with self.assertRaisesRegex(artifact.ArtifactError, "path"):
            artifact.verify(self.image, self.plan)

        artifact.prepare(self.image, self.plan, fixture=True, harness_created=True)
        raw = bytearray(self.image.read_bytes())
        raw[esp_offset + artifact.PAYLOAD_ALIGNMENT] ^= 1
        self.image.write_bytes(raw)
        with self.assertRaisesRegex(artifact.ArtifactError, "checksum"):
            artifact.verify(self.image, self.plan)

    def test_refuses_symlink_protected_artifact_and_oversize_payload(self):
        source = self.root / "artifact.bin"
        source.write_bytes(b"artifact")
        link = self.root / "link.bin"
        link.symlink_to(source)
        with self.assertRaisesRegex(artifact.ArtifactError, "symlink"):
            artifact.prepare(self.image, self.plan, artifact=link, harness_created=True)
        with self.assertRaisesRegex(artifact.ArtifactError, "exactly one"):
            artifact.prepare(self.image, self.plan, harness_created=True)
        oversized = self.root / "oversized.bin"
        oversized.write_bytes(b"x" * (artifact.MAX_ARTIFACT_BYTES + 1))
        with self.assertRaisesRegex(artifact.ArtifactError, "bounded range"):
            artifact.prepare(self.image, self.plan, artifact=oversized, harness_created=True)


if __name__ == "__main__":
    unittest.main()
