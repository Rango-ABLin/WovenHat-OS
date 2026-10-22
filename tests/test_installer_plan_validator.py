import importlib.util
import json
from pathlib import Path
import unittest


SCRIPT = Path(__file__).parents[1] / "scripts" / "validate-installer-plan.py"
SPEC = importlib.util.spec_from_file_location("installer_plan", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def valid_plan():
    return {
        "target_disk": {"path": "disk-1", "serial": "TARGET-001"},
        "inventory": [{"path": "disk-1", "serial": "TARGET-001", "size_bytes": 20 * 1024**3, "model": "test disk"}],
        "filesystem": "wovenfs",
        "partitions": [{"name": "root", "size_bytes": 8 * 1024**3}],
        "settings": {"user": {"username": "banda"}, "timezone": "Africa/Blantyre", "keyboard": "us", "network": {"mode": "dhcp"}},
    }


class InstallerPlanValidatorTests(unittest.TestCase):
    def assert_invalid(self, plan, text):
        with self.assertRaisesRegex(MODULE.PlanError, text):
            MODULE.build_plan(plan)

    def test_valid_plan_is_normalized_without_io(self):
        result = MODULE.build_plan(valid_plan())
        self.assertEqual(result["schema"], "woveninstaller-plan-v1")
        self.assertEqual(result["operations"]["disk_io"], "none")
        self.assertEqual(result["summary"]["partition_bytes"], 8 * 1024**3)

    def test_protected_and_ambiguous_targets_fail_closed(self):
        protected = valid_plan()
        protected["inventory"][0]["removable"] = True
        self.assert_invalid(protected, "protected")
        ambiguous = valid_plan()
        ambiguous["inventory"].append(dict(ambiguous["inventory"][0]))
        self.assert_invalid(ambiguous, "ambiguous")

    def test_overflow_and_insufficient_capacity_fail(self):
        overflow = valid_plan()
        overflow["partitions"][0]["size_bytes"] = 1 << 63
        self.assert_invalid(overflow, "64-bit")
        insufficient = valid_plan()
        insufficient["inventory"][0]["size_bytes"] = 2 * 1024**3
        self.assert_invalid(insufficient, "exceed")

    def test_invalid_settings_fail(self):
        for path, value, message in ((["user", "username"], "Bad User", "username"), (None, "Mars/Base", "timezone"), (None, "dvorak", "keyboard")):
            plan = valid_plan()
            if path is None:
                plan["settings"]["timezone" if message == "timezone" else "keyboard"] = value
            else:
                plan["settings"][path[0]][path[1]] = value
            self.assert_invalid(plan, message)
        plan = valid_plan()
        plan["settings"]["network"] = {"mode": "static", "address": "999.1.1.1/24", "gateway": "192.0.2.1", "dns": ["192.0.2.53"]}
        self.assert_invalid(plan, "network.address")
        plan = valid_plan()
        plan["settings"]["network"] = {"mode": "invalid"}
        self.assert_invalid(plan, "network.mode")

    def test_deterministic_canonical_output(self):
        first = MODULE.render(valid_plan())
        second = MODULE.render(json.loads(json.dumps(valid_plan(), sort_keys=True)))
        self.assertEqual(first, second)
        self.assertEqual(first, MODULE.render(valid_plan()))


if __name__ == "__main__":
    unittest.main()
