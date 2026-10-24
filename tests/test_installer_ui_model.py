import importlib.util
from pathlib import Path
import sys
import unittest


SCRIPT = Path(__file__).parents[1] / "scripts" / "installer-ui-model.py"
SPEC = importlib.util.spec_from_file_location("installer_ui_model", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class InstallerUiModelTests(unittest.TestCase):
    def advance_to_progress(self):
        state = MODULE.InstallerState()
        state = MODULE.transition(state, "begin")
        state = MODULE.transition(state, "target_selected", {"safe": True, "ambiguous": False, "summary": "Disk target TARGET-001"})
        state = MODULE.transition(state, "plan_reviewed", {"summary": "GPT / wovenfs / root 8 GiB"})
        state = MODULE.transition(state, "settings_saved", {"summary": "banda / Africa-Blantyre / us / dhcp"})
        return state

    def test_full_flow_requires_explicit_confirmation(self):
        state = self.advance_to_progress()
        with self.assertRaisesRegex(MODULE.UiError, "confirmation"):
            MODULE.transition(state, "progress", {"value": 1})
        state = MODULE.transition(state, "confirm_install", {"confirmed": True})
        state = MODULE.transition(state, "progress", {"value": 100, "message": "verified"})
        state = MODULE.transition(state, "succeed")
        self.assertEqual(state.screen, MODULE.Screen.SUCCESS)

    def test_illegal_order_and_unsafe_target_fail_closed(self):
        with self.assertRaises(MODULE.UiError):
            MODULE.transition(MODULE.InstallerState(), "settings_saved", {"summary": "x"})
        state = MODULE.transition(MODULE.InstallerState(), "begin")
        for payload in ({"safe": False, "ambiguous": False, "summary": "disk"}, {"safe": True, "ambiguous": True, "summary": "disk"}):
            with self.assertRaisesRegex(MODULE.UiError, "safe"):
                MODULE.transition(state, "target_selected", payload)

    def test_bounded_text_and_progress_are_enforced(self):
        state = MODULE.transition(MODULE.InstallerState(), "begin")
        with self.assertRaisesRegex(MODULE.UiError, "128"):
            MODULE.transition(state, "target_selected", {"safe": True, "ambiguous": False, "summary": "x" * 129})
        state = self.advance_to_progress()
        state = MODULE.transition(state, "confirm_install", {"confirmed": True})
        with self.assertRaises(MODULE.UiError):
            MODULE.transition(state, "progress", {"value": 101})
        state = MODULE.transition(state, "progress", {"value": 60})
        with self.assertRaisesRegex(MODULE.UiError, "backwards"):
            MODULE.transition(state, "progress", {"value": 59})

    def test_cancellation_and_failure_are_terminal_fail_closed_states(self):
        state = self.advance_to_progress()
        state = MODULE.transition(state, "request_cancel")
        self.assertEqual(state.screen, MODULE.Screen.CANCELLING)
        state = MODULE.transition(state, "cancelled")
        self.assertEqual(state.screen, MODULE.Screen.CANCELLED)
        with self.assertRaisesRegex(MODULE.UiError, "terminal"):
            MODULE.transition(state, "begin")

        failed = self.advance_to_progress()
        failed = MODULE.transition(failed, "fail", {"code": "PLAN_INVALID", "message": "plan rejected"})
        self.assertEqual(failed.screen, MODULE.Screen.FAILURE)
        with self.assertRaisesRegex(MODULE.UiError, "terminal"):
            MODULE.transition(failed, "confirm_install", {"confirmed": True})

    def test_cli_serializes_state_without_side_effects(self):
        result = MODULE.transition(MODULE.InstallerState(), "begin").as_dict()
        self.assertEqual(result["screen"], "target-selection")
        self.assertIn("destructive_confirmed", result)


if __name__ == "__main__":
    unittest.main()
