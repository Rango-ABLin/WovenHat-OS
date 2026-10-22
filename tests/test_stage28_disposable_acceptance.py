import hashlib
import importlib.util
import unittest


ROOT = __import__("pathlib").Path(__file__).parents[1]
SCRIPT = ROOT / "scripts" / "test-stage28-disposable-acceptance.py"
SPEC = importlib.util.spec_from_file_location("stage28_disposable_acceptance", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class Stage28DisposableAcceptanceTests(unittest.TestCase):
    def test_complete_pipeline_reports_all_digests_and_fail_closed_limits(self):
        result = MODULE.run_acceptance()
        self.assertTrue(result["valid"])
        self.assertEqual(result["pipeline"], "planner-layout-fat32-efi-executor")
        self.assertEqual(result["physical_access"], "none")
        self.assertEqual(result["release_certification"], "not-performed")
        self.assertRegex(result["image_sha256"], r"^[0-9a-f]{64}$")
        self.assertRegex(result["manifest_sha256"], r"^[0-9a-f]{64}$")
        self.assertRegex(result["layout"]["metadata_sha256"], r"^[0-9a-f]{64}$")
        self.assertRegex(result["layout"]["image_sha256"], r"^[0-9a-f]{64}$")
        self.assertRegex(result["filesystem"]["image_sha256"], r"^[0-9a-f]{64}$")
        self.assertRegex(result["efi"]["artifact_sha256"], r"^[0-9a-f]{64}$")
        self.assertRegex(result["executor"]["manifest_sha256"], r"^[0-9a-f]{64}$")
        self.assertTrue(result["executor"]["rollback_verified"])

    def test_repeated_runs_are_deterministic(self):
        first = MODULE.run_acceptance()
        second = MODULE.run_acceptance()
        for key in ("image_sha256", "manifest_sha256"):
            self.assertEqual(first[key], second[key])
        for section, key in (("layout", "metadata_sha256"), ("filesystem", "image_sha256"), ("efi", "artifact_sha256")):
            self.assertEqual(first[section][key], second[section][key])
        self.assertEqual(first["executor"]["manifest_sha256"], second["executor"]["manifest_sha256"])
        self.assertEqual(len(first["manifest_sha256"]), hashlib.sha256(b"x").digest_size * 2)


if __name__ == "__main__":
    unittest.main()
