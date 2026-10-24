#!/usr/bin/env python3
"""Pure WovenInstaller UI/state model.

This module models installer navigation only.  It accepts caller-provided
summaries and performs no filesystem, device, partition, format, bootloader,
or process I/O.  A future executor must not be reachable until the explicit
``confirm_install`` transition has succeeded.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass, replace
from enum import Enum
from typing import Any


MAX_TEXT = 128
MAX_ERROR = 256
MAX_PROGRESS_MESSAGE = 128


class UiError(ValueError):
    """A bounded payload or state transition is invalid."""


class Screen(str, Enum):
    WELCOME = "welcome"
    TARGET_SELECTION = "target-selection"
    PARTITION_REVIEW = "partition-review"
    SETTINGS = "settings"
    PROGRESS = "progress"
    CANCELLING = "cancelling"
    CANCELLED = "cancelled"
    SUCCESS = "success"
    FAILURE = "failure"


TERMINAL = frozenset((Screen.CANCELLED, Screen.SUCCESS, Screen.FAILURE))
ACTIVE = frozenset(
    (
        Screen.WELCOME,
        Screen.TARGET_SELECTION,
        Screen.PARTITION_REVIEW,
        Screen.SETTINGS,
        Screen.PROGRESS,
        Screen.CANCELLING,
    )
)


@dataclass(frozen=True)
class InstallerState:
    """Serializable UI state with no handles to external resources."""

    screen: Screen = Screen.WELCOME
    target_summary: str = ""
    plan_summary: str = ""
    settings_summary: str = ""
    progress: int = 0
    progress_message: str = ""
    destructive_confirmed: bool = False
    error_code: str = ""
    error_message: str = ""

    def as_dict(self) -> dict[str, Any]:
        return {
            "screen": self.screen.value,
            "target_summary": self.target_summary,
            "plan_summary": self.plan_summary,
            "settings_summary": self.settings_summary,
            "progress": self.progress,
            "progress_message": self.progress_message,
            "destructive_confirmed": self.destructive_confirmed,
            "error_code": self.error_code,
            "error_message": self.error_message,
        }


def _text(value: Any, label: str, limit: int = MAX_TEXT) -> str:
    if not isinstance(value, str) or not value or len(value) > limit:
        raise UiError(f"{label} must be 1..{limit} characters")
    if any(ord(char) < 0x20 for char in value):
        raise UiError(f"{label} contains a control character")
    return value


def _require_screen(state: InstallerState, *screens: Screen) -> None:
    if state.screen not in screens:
        expected = ", ".join(screen.value for screen in screens)
        raise UiError(f"event is not legal from {state.screen.value}; expected {expected}")


def _fail(state: InstallerState, payload: dict[str, Any]) -> InstallerState:
    _require_screen(state, *ACTIVE)
    code = _text(payload.get("code"), "error code", 64)
    message = _text(payload.get("message"), "error message", MAX_ERROR)
    return replace(state, screen=Screen.FAILURE, error_code=code, error_message=message)


def transition(state: InstallerState, event: str, payload: dict[str, Any] | None = None) -> InstallerState:
    """Apply one legal UI event and return a new immutable state.

    Terminal, cancelled, and failed states have no implicit recovery path.
    A caller must create a fresh ``InstallerState`` to start over.  The model
    never performs the operation represented by the progress screen.
    """

    data = payload or {}
    if state.screen in TERMINAL:
        raise UiError(f"terminal state {state.screen.value} rejects {event}")
    if event == "fail":
        return _fail(state, data)
    if event == "begin":
        _require_screen(state, Screen.WELCOME)
        return replace(state, screen=Screen.TARGET_SELECTION)
    if event == "target_selected":
        _require_screen(state, Screen.TARGET_SELECTION)
        if data.get("safe") is not True or data.get("ambiguous") is True:
            raise UiError("target selection must be explicitly safe and unambiguous")
        target = _text(data.get("summary"), "target summary")
        return replace(state, screen=Screen.PARTITION_REVIEW, target_summary=target)
    if event == "plan_reviewed":
        _require_screen(state, Screen.PARTITION_REVIEW)
        if state.target_summary == "":
            raise UiError("target selection is required before plan review")
        plan = _text(data.get("summary"), "plan summary")
        return replace(state, screen=Screen.SETTINGS, plan_summary=plan)
    if event == "settings_saved":
        _require_screen(state, Screen.SETTINGS)
        if state.plan_summary == "":
            raise UiError("partition plan review is required before settings")
        settings = _text(data.get("summary"), "settings summary")
        return replace(state, screen=Screen.PROGRESS, settings_summary=settings)
    if event == "confirm_install":
        _require_screen(state, Screen.PROGRESS)
        if data.get("confirmed") is not True:
            raise UiError("explicit destructive-operation confirmation is required")
        return replace(state, destructive_confirmed=True, progress_message="ready")
    if event == "progress":
        _require_screen(state, Screen.PROGRESS)
        if not state.destructive_confirmed:
            raise UiError("progress cannot start before explicit confirmation")
        value = data.get("value")
        if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 100:
            raise UiError("progress value must be an integer from 0 to 100")
        if value < state.progress:
            raise UiError("progress cannot move backwards")
        message = data.get("message", state.progress_message or "running")
        message = _text(message, "progress message", MAX_PROGRESS_MESSAGE)
        return replace(state, progress=value, progress_message=message)
    if event == "request_cancel":
        _require_screen(state, Screen.TARGET_SELECTION, Screen.PARTITION_REVIEW, Screen.SETTINGS, Screen.PROGRESS)
        return replace(state, screen=Screen.CANCELLING, progress_message="cancellation requested")
    if event == "cancelled":
        _require_screen(state, Screen.CANCELLING)
        return replace(state, screen=Screen.CANCELLED, destructive_confirmed=False, progress_message="cancelled")
    if event == "succeed":
        _require_screen(state, Screen.PROGRESS)
        if not state.destructive_confirmed or state.progress != 100:
            raise UiError("success requires confirmation and 100 percent progress")
        return replace(state, screen=Screen.SUCCESS, progress_message="complete")
    raise UiError(f"unknown event: {event}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", required=True)
    parser.add_argument("--state", type=json.loads, default="{}")
    parser.add_argument("--payload", type=json.loads, default="{}")
    args = parser.parse_args()
    try:
        raw = args.state
        state = InstallerState(
            screen=Screen(raw.get("screen", Screen.WELCOME.value)),
            target_summary=raw.get("target_summary", ""),
            plan_summary=raw.get("plan_summary", ""),
            settings_summary=raw.get("settings_summary", ""),
            progress=raw.get("progress", 0),
            progress_message=raw.get("progress_message", ""),
            destructive_confirmed=raw.get("destructive_confirmed", False),
            error_code=raw.get("error_code", ""),
            error_message=raw.get("error_message", ""),
        )
        print(json.dumps(transition(state, args.event, args.payload).as_dict(), sort_keys=True))
    except (TypeError, ValueError, UiError, KeyError) as exc:
        print(json.dumps({"valid": False, "error": str(exc)}, sort_keys=True))
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
