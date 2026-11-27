"""Exercise the Windows acceptance launcher's output-root and exit contract."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(os.name == "nt" and shutil.which("powershell.exe"), "Windows PowerShell required")
class Stage10LauncherTests(unittest.TestCase):
    def run_launcher(self, root, output_root, child_exit):
        shutil.copyfile(ROOT / "RUN-STAGE10.7.ps1", root / "RUN-STAGE10.7.ps1")
        (root / "target").mkdir()
        (root / "run-stage10-7-acceptance.ps1").write_text(
            'Write-Host "child output root: $env:WOVENHAT_OUTPUT_ROOT"\n'
            'Write-Host "child target: $env:CARGO_TARGET_DIR"\n'
            f"exit {child_exit}\n",
            encoding="utf-8",
        )
        env = os.environ.copy()
        if output_root is not None:
            env["WOVENHAT_OUTPUT_ROOT"] = str(output_root)
        else:
            env.pop("WOVENHAT_OUTPUT_ROOT", None)
        return subprocess.run(
            ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass",
             "-File", str(root / "RUN-STAGE10.7.ps1")],
            cwd=root, env=env, capture_output=True, text=True,
        )

    def test_existing_drive_override_is_used_and_logs_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "output"
            result = self.run_launcher(root, output, 0)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn(f"child output root: {output}", result.stdout)
            self.assertIn(f"child target: {output / 'target'}", result.stdout)
            logs = list((output / "audit-artifacts").glob("acceptance-*/acceptance-output.txt"))
            self.assertEqual(len(logs), 1)
            self.assertIn("child output root:", logs[0].read_text(encoding="utf-16"))

    def test_failing_child_is_not_reported_as_success(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "output"
            result = self.run_launcher(root, output, 17)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("acceptance failed with exit code 17", result.stderr)
            self.assertEqual(
                len(list((output / "audit-artifacts").glob("acceptance-*/acceptance-output.txt"))),
                1,
            )

    def test_relative_output_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_launcher(Path(directory), "relative-output", 0)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("available absolute drive or share", result.stderr)

    def test_default_output_uses_project_target_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.run_launcher(root, None, 0)
            expected = root / "target" / "stage10-output"
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn(f"child output root: {expected}", result.stdout)
            self.assertIn(f"child target: {expected / 'target'}", result.stdout)
