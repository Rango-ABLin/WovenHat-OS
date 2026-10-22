import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).parents[1] / "scripts" / "package-installer.py"
SPEC = importlib.util.spec_from_file_location("package_installer", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def image(path: Path, *, valid: bool = True) -> None:
    data = bytearray(4096)
    if valid:
        data[510:512] = b"\x55\xaa"
        data[512:520] = b"EFI PART"
    path.write_bytes(data)


class PackageInstallerTests(unittest.TestCase):
    def test_validates_gpt_file_image(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "boot.img"
            image(path)
            checks = MODULE.validate_uefi_disk_image(path)
            self.assertEqual(checks["sector_count"], 8)
            self.assertTrue(checks["protective_mbr"])
            self.assertTrue(checks["gpt_header"])

    def test_rejects_non_bootable_shape(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.img"
            image(path, valid=False)
            with self.assertRaises(ValueError):
                MODULE.validate_uefi_disk_image(path)

    def test_packages_with_digest_and_non_destructive_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.img"
            output = root / "package"
            image(source)
            result = MODULE.package(source, output, {"input": "explicit-image"}, "test")
            metadata = json.loads((result / MODULE.MANIFEST_NAME).read_text())
            artifact = result / metadata["artifact"]["file"]
            self.assertEqual(MODULE.sha256(source), metadata["artifact"]["sha256"])
            self.assertEqual(MODULE.sha256(artifact), metadata["artifact"]["sha256"])
            self.assertEqual(metadata["physical_disk_access"], "none")
            self.assertEqual(metadata["destructive_partitioning"], "not-implemented")
            with self.assertRaises(FileExistsError):
                MODULE.package(source, output, {"input": "explicit-image"}, "test")


if __name__ == "__main__":
    unittest.main()
