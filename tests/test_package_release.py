"""Host contract checks for the release packager's acceptance markers."""

from pathlib import Path
import unittest


class ReleasePackagerContractTests(unittest.TestCase):
    def test_requires_each_system_app_cpu_gate(self) -> None:
        source = (Path(__file__).parents[1] / "scripts" / "package-release.py").read_text(
            encoding="utf-8"
        )

        self.assertIn("required_checks", source)
        self.assertIn("set(required)", source)
        self.assertIn("apps + ('woven-package', 'woven-ai', 'woven-shell')", source)
        self.assertIn("required_checks", source)
        self.assertIn("test-uefi-iso-qemu.py", source)
