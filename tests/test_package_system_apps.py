import importlib.util
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "package-system-apps.py"
SPEC = importlib.util.spec_from_file_location("package_system_apps", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class SystemAppPackagingTests(unittest.TestCase):
    def test_existing_entries_are_preserved_and_installer_is_appended(self):
        ids = [app["id"] for app in MODULE.APPS]
        self.assertEqual(ids[:10], [
            "woven-calculator", "woven-terminal", "woven-files",
            "woven-monitor", "woven-settings", "woven-security",
            "woven-editor", "woven-images", "woven-update", "woven-store",
        ])
        self.assertEqual(ids[-1], "woven-installer")

    def test_shell_catalog_capacity_covers_all_signed_system_apps(self):
        shell_source = (ROOT / "apps" / "woven-shell" / "src" / "main.rs").read_text()
        match = re.search(r"^const MAX_APPS: usize = ([0-9]+);$", shell_source, re.MULTILINE)
        self.assertIsNotNone(match, "WovenShell catalog capacity declaration missing")
        capacity = int(match.group(1))
        self.assertGreaterEqual(
            capacity, len(MODULE.APPS),
            "WovenShell must enumerate every signed packaged application",
        )

    def test_installer_catalog_contract_is_non_privileged_display_only(self):
        installer = next(app for app in MODULE.APPS if app["id"] == "woven-installer")
        MODULE.validate_app_definition(installer)
        self.assertEqual(installer["install_path"], "/bin/wovenhat-installer")
        self.assertEqual(installer["exec"], "/bin/wovenhat-installer")
        self.assertEqual(installer["capabilities"], ())
        self.assertFalse(installer["privileged"])
        self.assertEqual(installer["interface"], "display-only")

    def test_forbidden_capability_is_rejected(self):
        installer = next(app for app in MODULE.APPS if app["id"] == "woven-installer")
        invalid = dict(installer, capabilities=("disk",))
        with self.assertRaises(RuntimeError):
            MODULE.validate_app_definition(invalid)


if __name__ == "__main__":
    unittest.main()
