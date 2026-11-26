"""Host contract checks for the release packager's acceptance markers."""

import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).parents[1]
PACKAGER = ROOT / "scripts" / "package-release.py"
SPEC = importlib.util.spec_from_file_location("package_release", PACKAGER)
assert SPEC is not None and SPEC.loader is not None
PACKAGE_RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE_RELEASE)


class ReleasePackagerContractTests(unittest.TestCase):
    def test_requires_each_system_app_cpu_gate(self) -> None:
        source = PACKAGER.read_text(encoding="utf-8")

        self.assertNotIn("required.add('system-apps-boot')", source)
        self.assertIn("required |= {f'system-apps-boot-{cpus}' for cpus in (1, 2, 4)}", source)
        self.assertIn("apps + ('woven-package', 'woven-ai', 'woven-shell')", source)

    def test_validation_manifest_records_source_commit(self) -> None:
        source = (ROOT / "scripts" / "test-release.py").read_text(encoding="utf-8")
        self.assertIn("source_commit = subprocess.check_output(", source)
        self.assertIn("dict(source_commit=source_commit, image=", source)

    def test_requires_exact_commit_and_clean_worktree(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.name", "Release Test"], cwd=repo, check=True)
            subprocess.run(
                ["git", "config", "user.email", "release-test@example.test"],
                cwd=repo,
                check=True,
            )
            source = repo / "source.txt"
            source.write_text("accepted source\n", encoding="utf-8")
            subprocess.run(["git", "add", "source.txt"], cwd=repo, check=True)
            subprocess.run(["git", "commit", "-q", "-m", "accepted source"], cwd=repo, check=True)
            commit = subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=repo, text=True
            ).strip()
            manifest = {"source_commit": commit}

            PACKAGE_RELEASE.require_validated_source(repo, manifest)

            source.write_text("changed source\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "dirty working tree"):
                PACKAGE_RELEASE.require_validated_source(repo, manifest)

            source.write_text("accepted source\n", encoding="utf-8")
            untracked = repo / "untracked.txt"
            untracked.write_text("not committed\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "dirty working tree"):
                PACKAGE_RELEASE.require_validated_source(repo, manifest)
            untracked.unlink()

            manifest["source_commit"] = "0" * 40
            with self.assertRaisesRegex(ValueError, "commit changed"):
                PACKAGE_RELEASE.require_validated_source(repo, manifest)
