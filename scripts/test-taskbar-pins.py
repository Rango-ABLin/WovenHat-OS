#!/usr/bin/env python3
"""Fail-closed, real-QEMU taskbar pinning qualification.

Exercises the Phase B pinned-application wiring end to end on the same
hardware-demo image the desktop qualification uses: a left-click on a
pinned entry launches its app, a right-click on any taskbar entry pins or
unpins that app in-session, a pinned app keeps its taskbar slot after its
window closes, and the group re-flows on every state change.

Journey (PS/2 pointer moves are in the guest's logical 640x400 space; the
scanout is 1280x800, the logical 2x scale):

  1. left-click  the Files pin            -> launched + window opened
  2. right-click the Files window entry   -> "Unpinned WovenFiles", re-flow
  3. right-click the Files entry at its
     new tail position                    -> "Pinned WovenFiles"
  4. Escape closes the focused Files
     window                               -> "window closed: Files", the
                                              pinned entry keeps its slot
  5. right-click the pinned-not-running
     Files entry                          -> "Unpinned WovenFiles" again,
                                              proving the pin survived

Every step must publish its serial marker, and steps 1, 2, 4 and 5 must
also repaint the taskbar strip on the scanout within the frame budget.
Marker waits are occurrence-counted, so a repeated "Unpinned WovenFiles"
cannot pass on its earlier occurrence. Nothing here asserts a pin
survives a reboot: the shell holds no settings-write path, and this gate
does not pretend otherwise.

Return codes: 0 pass, 1 a guest check failed, 2 unsupported environment.

Reuses the desktop-qualification harness's QMP plumbing, build steps and
evidence conventions by importing it as a module. Like that harness, this
gate owns the QEMU run; do not run two acceptance chains at once (shared
build/image paths).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]

_spec = importlib.util.spec_from_file_location(
    "desktop_qualification", ROOT / "scripts" / "test-desktop-qualification.py"
)
dq = importlib.util.module_from_spec(_spec)
assert _spec.loader is not None
_spec.loader.exec_module(dq)

# Logical pointer space and the shell's taskbar constants (tokens.rs /
# main.rs): bar height 24, Start button 58 wide, medium entries 48 wide,
# 4 px gaps, Start-to-entries padding 8.
LOGICAL_WIDTH = 640
LOGICAL_HEIGHT = 400
BAR_HEIGHT = 24
START_WIDTH = 58
ENTRY_WIDTH = 48
ENTRY_GAP = 4
# The taskbar strip in physical scanout pixels (the framebuffer is the
# logical desktop at 2x).
STRIP_TOP = 800 - 2 * BAR_HEIGHT
STRIP_HEIGHT = 2 * BAR_HEIGHT


def group_width(entries: int) -> int:
    """The shell's `taskbar_group_width`, mirrored for hit targeting."""
    if entries <= 0:
        return START_WIDTH
    return START_WIDTH + 8 + entries * ENTRY_WIDTH + (entries - 1) * ENTRY_GAP


def entry_center(index: int, entries: int) -> tuple[int, int]:
    """Logical center of taskbar entry `index` when the group has `entries`."""
    start_x = (LOGICAL_WIDTH - group_width(entries)) // 2
    x = start_x + START_WIDTH + 8 + index * (ENTRY_WIDTH + ENTRY_GAP) + ENTRY_WIDTH // 2
    y = LOGICAL_HEIGHT - BAR_HEIGHT + BAR_HEIGHT // 2
    return x, y


def right_click(qmp: dq.Qmp) -> None:
    """A PS/2 right-button press and release through the same event path
    the qualification's left-click helper uses."""
    qmp.command("input-send-event", {"events": [
        {"type": "btn", "data": {"down": True, "button": "right"}},
    ]})
    time.sleep(0.08)
    qmp.command("input-send-event", {"events": [
        {"type": "btn", "data": {"down": False, "button": "right"}},
    ]})


def wait_marker_count(
    serial: Path,
    marker: str,
    expected: int,
    deadline: float,
    process: subprocess.Popen[Any],
    phase: str,
    timings: dict[str, float],
    started: float,
) -> None:
    """Wait until `marker` has appeared `expected` times in the serial log.

    A substring wait would pass instantly on a repeated marker's earlier
    occurrence, so pin/unpin steps count occurrences instead.
    """
    while time.monotonic() < deadline:
        if dq.read_serial(serial).count(marker) >= expected:
            timings[phase] = (time.monotonic() - started) * 1000.0
            return
        if process.poll() is not None:
            raise dq.GateFailure(
                f"QEMU exited before marker {marker!r} x{expected} for {phase}"
            )
        time.sleep(0.05)
    raise dq.GateFailure(
        f"timed out waiting for marker {marker!r} x{expected} for {phase}"
    )


def wait_strip_change(
    qmp: dq.Qmp,
    output: Path,
    reference: bytes,
    name: str,
    timings: dict[str, float],
    deadline: float,
) -> tuple[int, int, bytes]:
    """Retain scanouts until the taskbar strip materially changes against
    `reference`; keeps the frame as evidence and returns its pixels."""
    frame = output / f"{name}.ppm"
    return dq.capture_until_changed(
        qmp, frame, reference, deadline, timings, name, 20,
        dq.region_changed(reference, 0, STRIP_TOP, 1280, STRIP_HEIGHT, 20),
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", type=Path, default=Path(dq.DEFAULT_QEMU))
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--memory", default="5G")
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument("--input-timeout", type=float, default=60.0)
    parser.add_argument("--frame-timeout", type=float, default=15.0)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    if args.timeout <= 0 or args.input_timeout <= 0 or args.frame_timeout <= 0:
        parser.error("timeouts must be positive")
    requested = args.output or ROOT / "audit-artifacts" / f"taskbar-pins-{time.time_ns()}"
    try:
        output = dq.create_output(ROOT, requested)
    except ValueError as error:
        parser.error(str(error))
    qemu = args.qemu.resolve()
    firmware = dq.resolve_firmware(qemu, args.firmware.resolve() if args.firmware else None)
    base: dict[str, Any] = {
        "schema": 1,
        "classification": "real QEMU taskbar pinning qualification; no physical certification",
        "qemu": str(qemu),
        "firmware": str(firmware.resolve()),
        "cpus": args.cpus,
        "physical_hardware_touched": False,
        "reboot_requested": False,
        "output": str(output),
    }
    blockers = dq.preflight(qemu, firmware)
    if blockers:
        base.update({"status": "UNSUPPORTED", "pass": False, "unsupported_blockers": blockers})
        (output / "result.json").write_text(json.dumps(base, indent=2) + "\n", encoding="utf-8")
        print(f"UNSUPPORTED: {', '.join(blockers)}; evidence: {output}", file=sys.stderr)
        return 2

    timings: dict[str, float] = {}
    screenshots: list[str] = []
    serial: Path | None = None
    process: subprocess.Popen[Any] | None = None
    try:
        image = dq.build_hardware_demo(output)
        serial = output / "serial.log"
        qmp_port = dq.free_port()
        command = [
            str(qemu), "-accel", "tcg,tb-size=128",
            "-machine", "pc", "-m", args.memory,
            "-smp", str(args.cpus), "-serial", f"file:{serial}",
            "-no-reboot", "-qmp", f"tcp:127.0.0.1:{qmp_port},server=on,wait=off",
            "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
            "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
            "-device", "virtio-blk-pci,drive=boot,bootindex=1",
            "-display", "none", "-vga", "std",
        ]
        (output / "qemu-command.json").write_text(
            json.dumps(command, indent=2) + "\n", encoding="utf-8"
        )
        started = time.monotonic()
        with (output / "qemu.log").open("w", encoding="utf-8") as qemu_log:
            flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
            process = subprocess.Popen(
                command, cwd=ROOT, stdout=qemu_log, stderr=subprocess.STDOUT,
                creationflags=flags,
            )
            qmp = dq.Qmp(
                qmp_port, time.monotonic() + 30, output / "qmp.jsonl", process, None
            )
            wait = lambda marker, phase, since: wait_marker_count(
                serial, marker, 1, since + args.input_timeout, process, phase, timings, since
            )
            dq.wait_marker(
                serial, "[SESSION] WovenShell desktop started", started + args.timeout,
                process, "desktop_session_startup_ms", timings, started,
            )
            dq.wait_marker(
                serial, "woven-shell: desktop ready", started + args.timeout,
                process, "desktop_ready_ms", timings, started,
            )
            # The same settling window the desktop qualification gives the
            # lock screen before injecting the PIN.
            time.sleep(8.0)
            sign_in = time.monotonic()
            for _ in range(3):
                for key in ("1", "2", "3", "4", "ret"):
                    qmp.key(key, 150)
                    time.sleep(0.4)
                if "woven-shell: signed in: ada" in dq.read_serial(serial):
                    break
                time.sleep(0.5)
            dq.wait_marker(
                serial, "woven-shell: signed in: ada", sign_in + args.input_timeout,
                process, "sign_in_input_ms", timings, sign_in,
            )
            frame_ready = time.monotonic()
            dq.wait_marker(
                serial, "woven-shell: desktop frame ready", frame_ready + args.input_timeout,
                process, "desktop_frame_ready_ms", timings, frame_ready,
            )
            baseline = output / "pins-baseline.ppm"
            _, _, reference = dq.capture_frame(
                qmp, baseline, time.monotonic() + args.frame_timeout, timings, "pins_baseline"
            )
            screenshots.append(baseline.name)

            # The pointer starts at the logical centre, as in the desktop
            # qualification's Files journey.
            pointer = (LOGICAL_WIDTH // 2, LOGICAL_HEIGHT // 2)

            def move_to(target: tuple[int, int]) -> None:
                nonlocal pointer
                dx, dy = target[0] - pointer[0], target[1] - pointer[1]
                qmp.relative_mouse(dx, dy)
                time.sleep(0.3)
                time.sleep(0.5)
                pointer = target

            # Step 1: left-click the Files pin. No window is open, so the
            # pinned entry must launch the app.
            step = time.monotonic()
            move_to(entry_center(0, 3))
            qmp.click()
            wait_marker_count(
                serial, "woven-shell: launched WovenFiles", 1, step + args.input_timeout,
                process, "pins_launch_ms", timings, step,
            )
            wait_marker_count(
                serial, "woven-shell: window opened: Files", 1, step + args.input_timeout,
                process, "pins_window_open_ms", timings, step,
            )
            _, _, after_launch = wait_strip_change(
                qmp, output, reference, "pins-step1-launched", timings,
                time.monotonic() + args.frame_timeout,
            )
            screenshots.append("pins-step1-launched.ppm")

            # Step 2: right-click the Files window's entry at the pin's
            # position. The pin is released; the window keeps its entry,
            # now in the un-pinned tail.
            step = time.monotonic()
            right_click(qmp)
            wait_marker_count(
                serial, "woven-shell: Unpinned WovenFiles", 1, step + args.input_timeout,
                process, "pins_unpin_ms", timings, step,
            )
            _, _, after_unpin = wait_strip_change(
                qmp, output, after_launch, "pins-step2-unpinned", timings,
                time.monotonic() + args.frame_timeout,
            )
            screenshots.append("pins-step2-unpinned.ppm")

            # Step 3: right-click the same window's entry at its new tail
            # position. The layout does not change (the pin re-enters at the
            # position the window already occupies), so the marker is the
            # assertion; the frame is still kept as evidence.
            step = time.monotonic()
            move_to(entry_center(2, 3))
            right_click(qmp)
            wait_marker_count(
                serial, "woven-shell: Pinned WovenFiles", 1, step + args.input_timeout,
                process, "pins_repin_ms", timings, step,
            )
            repin = output / "pins-step3-repinned.ppm"
            _, _, after_repin = dq.capture_frame(
                qmp, repin, time.monotonic() + args.frame_timeout, timings, "pins_repin"
            )
            screenshots.append(repin.name)

            # Step 4: close the focused Files window. A pinned app keeps
            # its slot; only the running indicator disappears.
            step = time.monotonic()
            qmp.key("esc", 90)
            wait_marker_count(
                serial, "woven-shell: keyboard close requested", 1, step + args.input_timeout,
                process, "pins_close_request_ms", timings, step,
            )
            wait_marker_count(
                serial, "woven-shell: window closed: Files", 1, step + args.input_timeout,
                process, "pins_window_closed_ms", timings, step,
            )
            _, _, after_close = wait_strip_change(
                qmp, output, after_repin, "pins-step4-closed", timings,
                time.monotonic() + args.frame_timeout,
            )
            screenshots.append("pins-step4-closed.ppm")

            # Step 5: right-click the pinned-not-running Files entry. Its
            # second "Unpinned" occurrence proves the pin survived the
            # window close, and the group must re-flow (3 entries -> 2).
            step = time.monotonic()
            right_click(qmp)
            wait_marker_count(
                serial, "woven-shell: Unpinned WovenFiles", 2, step + args.input_timeout,
                process, "pins_unpin_again_ms", timings, step,
            )
            wait_strip_change(
                qmp, output, after_close, "pins-step5-unpinned-again", timings,
                time.monotonic() + args.frame_timeout,
            )
            screenshots.append("pins-step5-unpinned-again.ppm")

            result = dict(base)
            result.update({
                "status": "PASS",
                "pass": True,
                "failures": [],
                "image": str(image),
                "screenshots": screenshots,
                "timings_ms": {**timings},
                "pins_persistence": {
                    "across_reboot": False,
                    "reason": "the shell process holds FileRead only; no settings-write path exists",
                },
            })
            (output / "result.json").write_text(
                json.dumps(result, indent=2) + "\n", encoding="utf-8"
            )
            print(f"Taskbar pinning: PASS ({args.cpus} CPU). Evidence: {output}")
            return 0
    except dq.GateFailure as failure:
        result = dict(base)
        result.update({
            "status": "FAIL",
            "pass": False,
            "failures": [str(failure)],
            "screenshots": screenshots,
            "timings_ms": {**timings},
        })
        (output / "result.json").write_text(
            json.dumps(result, indent=2) + "\n", encoding="utf-8"
        )
        print(f"FAILED: {failure}; evidence: {output}", file=sys.stderr)
        return 1
    except dq.UnsupportedQualification as unsupported:
        result = dict(base)
        result.update({
            "status": "UNSUPPORTED",
            "pass": False,
            "failures": [str(unsupported)],
            "screenshots": screenshots,
            "timings_ms": {**timings},
        })
        (output / "result.json").write_text(
            json.dumps(result, indent=2) + "\n", encoding="utf-8"
        )
        print(f"UNSUPPORTED: {unsupported}; evidence: {output}", file=sys.stderr)
        return 2
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()


if __name__ == "__main__":
    raise SystemExit(main())
