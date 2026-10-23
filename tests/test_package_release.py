"""Host contract checks for the release packager's acceptance markers."""

from pathlib import Path
import unittest


class ReleasePackagerContractTests(unittest.TestCase):
    def test_requires_each_system_app_cpu_gate(self) -> None:
        source = (Path(__file__).parents[1] / "scripts" / "package-release.py").read_text(
            encoding="utf-8"
        )

        self.assertNotIn("required.add('system-apps-boot')", source)
        self.assertIn("required |= {f'system-apps-boot-{cpus}' for cpus in (1, 2, 4)}", source)
        self.assertIn("shell_root = root / 'apps' / 'woven-shell'", source)
