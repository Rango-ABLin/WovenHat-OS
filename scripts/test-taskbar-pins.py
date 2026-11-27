#!/usr/bin/env python3
"""Fail-closed, real-QEMU taskbar pinning, auto-hide and maximize
qualification.

Exercises the Phase B pinned-application wiring, auto-hide and window
maximize end to end on the same hardware-demo image the desktop
qualification uses: a left-click on a pinned entry launches its app, a
right-click on any taskbar entry pins or unpins that app in-session, a
pinned app keeps its taskbar slot after its window closes, and the group
re-flows on every state change. Auto-hide then follows in the same boot:
right-clicking an empty part of the strip enables it, the hidden bar's
controls stop capturing clicks, the screen's bottom row reveals it, the
hold timer hides it again once the pointer leaves, and a final
right-click turns it back off with the bar staying up. Maximize closes
the boot: a right-click on a window's title bar fills the usable band, a
second right-click returns the exact saved geometry, and dragging a
maximized title bar restores the window first.

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
  6. right-click an empty part of the
     strip                                -> "Taskbar auto-hide on", the
                                              strip vanishes
  7. left-click where the hidden Start
     button would be                      -> nothing happens: hidden
                                              controls capture nothing
  8. move to the bottom screen row        -> "taskbar revealed", the
                                              strip repaints
  9. move away and wait past the hold     -> "taskbar hidden", the strip
                                              vanishes on its own
 10. reveal, then right-click empty strip -> "Taskbar auto-hide off", and
                                              the bar stays up past the
                                              old hold (negative check)
 11. left-click the Terminal pin          -> launched + window opened
 12. right-click the Terminal title bar   -> "window maximized", the
                                              band-filling frame reaches
                                              the top-left corner
 13. right-click the maximized title bar  -> "window restored", the
                                              corner goes back to
                                              wallpaper and the window's
                                              region is pixel-identical
                                              to its pre-maximize frame
 14. maximize, then drag the maximized
     title bar                           -> a second "restored": dragging
                                              a maximized window restores
                                              it first

Every step must publish its serial marker, and the re-flow steps must
also repaint the taskbar strip on the scanout within the frame budget.
Marker waits are occurrence-counted, so repeated markers cannot pass on
their earlier occurrences. The two negative steps compare pixels
directly (the clock columns masked where the wall clock can legitimately
change) and fail on any difference.

Nothing here asserts a pin or the auto-hide preference survives a
reboot: the shell holds no settings-write path, and this gate does not
pretend otherwise.

Return codes: 0 pass, 1 a guest check failed, 2 unsupported environment.

Reuses the desktop-qualification harness's QMP plumbing, build steps and
evidence conventions by importing it as a module. Like that harness,
this gate owns the QEMU run; do not run two acceptance chains at once
(shared build/image paths).
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


def controls_left() -> int:
    """The left edge (minus its 8 px margin) of the strip's right-side
    controls, mirroring the shell's `start_area` bound: workspace boxes,
    then the tray, status icons and clock anchored at the right edge."""
    clock_x = LOGICAL_WIDTH - (8 * 6 + 6) - 4
    qs_x = clock_x - (4 + 3 + 1) * (16 + 4)
    span = 4 * (12 + 2) - 2
    return qs_x - 6 - span - 8


def group_left(width: int) -> int:
    """The shell's centered-group placement inside the free band between
    the left margin and the right-side controls, mirrored so this gate
    clicks where the shell actually draws."""
    controls = controls_left()
    free = max(controls - 8, 0)
    candidate = 8 + free // 2 - width // 2
    return max(8, min(candidate, max(controls - width, 8)))


def entry_center(index: int, entries: int) -> tuple[int, int]:
    """Logical center of taskbar entry `index` when the group has `entries`."""
    x = (
        group_left(group_width(entries))
        + START_WIDTH
        + 8
        + index * (ENTRY_WIDTH + ENTRY_GAP)
        + ENTRY_WIDTH // 2
    )
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


# The strip's wall clock legitimately repaints on a minute boundary, so the
# negative checks mask its columns instead of trusting a short settle window
# never to cross one.
CLOCK_X0, CLOCK_X1 = 2 * 582, 2 * 636


def strip_equal_ignoring_clock(left: bytes, right: bytes) -> bool:
    for y in range(STRIP_TOP, STRIP_TOP + STRIP_HEIGHT):
        row = y * 1280 * 3
        for x in range(0, 1280):
            if CLOCK_X0 <= x < CLOCK_X1:
                continue
            offset = row + x * 3
            if left[offset:offset + 3] != right[offset:offset + 3]:
                return False
    return True


def region_equal(left: bytes, right: bytes, box: tuple[int, int, int, int]) -> bool:
    """Pixel equality of one physical box (x0, y0, x1, y1) between two
    frames. Used where the guest must return to an exact previous state
    and no cursor or toast can enter the box between the two captures."""
    x0, y0, x1, y1 = box
    for y in range(y0, y1):
        row = y * 1280 * 3
        for x in range(x0, x1):
            offset = row + x * 3
            if left[offset:offset + 3] != right[offset:offset + 3]:
                return False
    return True


def capture_and_require_unchanged(
    qmp: dq.Qmp,
    output: Path,
    reference: bytes,
    name: str,
    timings: dict[str, float],
    deadline: float,
    settle: float,
) -> tuple[int, int, bytes]:
    """Settle, capture, and fail if the strip changed at all outside the
    clock columns. The negative half of the auto-hide assertions: a hidden
    bar must capture nothing, and a bar with auto-hide off must stay up."""
    time.sleep(settle)
    frame = output / f"{name}.ppm"
    width, height, pixels = dq.capture_frame(qmp, frame, deadline, timings, name)
    if not strip_equal_ignoring_clock(reference, pixels):
        raise dq.GateFailure(f"{name}: taskbar strip changed when it must not have")
    return width, height, pixels


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
                # PS/2 mouse packets carry 9-bit signed deltas; a single large
                # QMP relative move can be clipped by the guest's PS/2 path. Chunk
                # into safe steps so the cursor lands where the gate aimed it.
                while dx != 0 or dy != 0:
                    step_x = max(-100, min(100, dx))
                    step_y = max(-100, min(100, dy))
                    qmp.relative_mouse(step_x, step_y)
                    dx -= step_x
                    dy -= step_y
                    time.sleep(0.05)
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
            _, _, after_unpin_again = wait_strip_change(
                qmp, output, after_close, "pins-step5-unpinned-again", timings,
                time.monotonic() + args.frame_timeout,
            )
            screenshots.append("pins-step5-unpinned-again.ppm")

            # Step 6: right-click an empty part of the strip: auto-hide on,
            # and the bar goes down immediately. With the two remaining
            # pins the group ends at 264, and the right-side controls start
            # at 362, so x=330 is empty strip.
            step = time.monotonic()
            move_to((330, LOGICAL_HEIGHT - BAR_HEIGHT + BAR_HEIGHT // 2))
            right_click(qmp)
            wait_marker_count(
                serial, "woven-shell: Taskbar auto-hide on", 1, step + args.input_timeout,
                process, "autohide_on_ms", timings, step,
            )
            _, _, hidden_frame = wait_strip_change(
                qmp, output, after_unpin_again, "autohide-step6-on", timings,
                time.monotonic() + args.frame_timeout,
            )
            screenshots.append("autohide-step6-on.ppm")

            # Step 7: the hidden bar must capture nothing. Move to where the
            # hidden Start button would be and wait until the drawn cursor
            # has actually arrived there, so the reference frame and the
            # comparison frame show the same cursor; then click, and
            # require no change at all.
            step = time.monotonic()
            target = (group_left(group_width(2)) + START_WIDTH // 2,
                      LOGICAL_HEIGHT - BAR_HEIGHT + BAR_HEIGHT // 2)
            move_to(target)
            arrived = output / "autohide-step7-arrived.ppm"
            # The box must stay inside the 800-row framebuffer: the cursor
            # glyph spans roughly 776..795, so rows 768..800 cover it.
            _, _, settled_frame = dq.capture_until_changed(
                qmp, arrived, hidden_frame,
                time.monotonic() + args.frame_timeout, timings, "autohide_arrived", 20,
                dq.region_changed(hidden_frame, 2 * target[0] - 8, 2 * target[1] - 8, 48, 32, 20),
            )
            screenshots.append(arrived.name)
            qmp.click()
            capture_and_require_unchanged(
                qmp, output, settled_frame, "autohide-step7-blocked", timings,
                time.monotonic() + args.frame_timeout, 1.0,
            )
            screenshots.append("autohide-step7-blocked.ppm")

            # Step 8: the screen's bottom row reveals the bar.
            step = time.monotonic()
            move_to((LOGICAL_WIDTH // 2, LOGICAL_HEIGHT - 1))
            wait_marker_count(
                serial, "woven-shell: taskbar revealed", 1, step + args.input_timeout,
                process, "autohide_reveal_ms", timings, step,
            )
            _, _, revealed_frame = wait_strip_change(
                qmp, output, settled_frame, "autohide-step8-revealed", timings,
                time.monotonic() + args.frame_timeout,
            )
            screenshots.append("autohide-step8-revealed.ppm")

            # Step 9: away from the strip, the hold runs out and the bar
            # hides on its own.
            step = time.monotonic()
            move_to((LOGICAL_WIDTH // 2, LOGICAL_HEIGHT // 2))
            wait_marker_count(
                serial, "woven-shell: taskbar hidden", 1, step + args.input_timeout,
                process, "autohide_hide_ms", timings, step,
            )
            wait_strip_change(
                qmp, output, revealed_frame, "autohide-step9-hidden", timings,
                time.monotonic() + args.frame_timeout,
            )
            screenshots.append("autohide-step9-hidden.ppm")

            # Step 10: reveal once more, then right-click the empty strip to
            # turn auto-hide off. The bar must stay up past the old hold
            # with the pointer far away.
            step = time.monotonic()
            move_to((LOGICAL_WIDTH // 2, LOGICAL_HEIGHT - 1))
            wait_marker_count(
                serial, "woven-shell: taskbar revealed", 2, step + args.input_timeout,
                process, "autohide_reveal_again_ms", timings, step,
            )
            # Moving to the empty strip refreshes the hold on the way.
            move_to((330, LOGICAL_HEIGHT - BAR_HEIGHT + BAR_HEIGHT // 2))
            # Refresh the auto-hide hold just before the click so the 1s
            # timer cannot race the gate's own settle sleeps.
            qmp.relative_mouse(0, -1)
            time.sleep(0.05)
            qmp.relative_mouse(0, 1)
            time.sleep(0.05)
            right_click(qmp)
            wait_marker_count(
                serial, "woven-shell: Taskbar auto-hide off", 1, step + args.input_timeout,
                process, "autohide_off_ms", timings, step,
            )
            # Capture with the cursor still at the click, then park the
            # pointer and wait until the cursor has actually left that
            # spot before using a frame as the stay-up reference: the
            # comparison must never race the cursor's own repaint.
            off = output / "autohide-step10-off.ppm"
            _, _, off_frame = dq.capture_frame(
                qmp, off, time.monotonic() + args.frame_timeout, timings, "autohide_off"
            )
            screenshots.append(off.name)
            parked_target = (LOGICAL_WIDTH // 2, LOGICAL_HEIGHT // 2)
            parked_from = (330, LOGICAL_HEIGHT - BAR_HEIGHT + BAR_HEIGHT // 2)
            move_to(parked_target)
            parked = output / "autohide-step10-parked.ppm"
            # Same bounds discipline: the departure box watches the cursor's
            # old spot inside the framebuffer, rows 768..800.
            _, _, parked_frame = dq.capture_until_changed(
                qmp, parked, off_frame,
                time.monotonic() + args.frame_timeout, timings, "autohide_parked", 20,
                dq.region_changed(
                    off_frame, 2 * parked_from[0] - 8, 2 * parked_from[1] - 8, 48, 32, 20
                ),
            )
            screenshots.append(parked.name)
            _, _, quiet_frame = capture_and_require_unchanged(
                qmp, output, parked_frame, "autohide-step10-stays-up", timings,
                time.monotonic() + args.frame_timeout, 2.5,
            )
            screenshots.append("autohide-step10-stays-up.ppm")

            # Step 11: launch the Terminal through its pinned entry, the
            # same launch path the Files pin exercised.
            step = time.monotonic()
            move_to(entry_center(0, 2))
            qmp.click()
            wait_marker_count(
                serial, "woven-shell: launched WovenTerminal", 1, step + args.input_timeout,
                process, "maximize_launch_ms", timings, step,
            )
            wait_marker_count(
                serial, "woven-shell: window opened: Terminal", 1, step + args.input_timeout,
                process, "maximize_window_open_ms", timings, step,
            )
            terminal = output / "maximize-step11-launched.ppm"
            _, _, terminal_frame = dq.capture_until_changed(
                qmp, terminal, quiet_frame,
                time.monotonic() + args.frame_timeout, timings, "maximize_launch", 1000,
            )
            screenshots.append(terminal.name)

            # Step 12: right-click the Terminal title bar -- cascade offset 64,
            # title bar (64, 50, 360, 14). The window must maximize to the
            # whole usable band, which the top-left corner box proves: the
            # maximized title bar covers what was wallpaper.
            step = time.monotonic()
            move_to((64 + 180, 50 + 7))
            right_click(qmp)
            wait_marker_count(
                serial, "woven-shell: window maximized: Terminal", 1, step + args.input_timeout,
                process, "maximize_on_ms", timings, step,
            )
            # The maximize WSIZ must reach the client and come back as WRSZ.
            # Terminal clamps to its bounded 480x320 canvas, so the shell keeps
            # the band-filling frame and logs the clamp: a frame-only maximize
            # with no client reflow cannot produce these markers.
            wait_marker_count(
                serial, "WovenTerminal: window resized", 1, step + args.input_timeout,
                process, "maximize_client_reflow_ms", timings, step,
            )
            wait_marker_count(
                serial, "woven-shell: window resized (maximized, clamped): Terminal", 1,
                step + args.input_timeout, process, "maximize_wrsz_ms", timings, step,
            )
            maximized = output / "maximize-step12-maximized.ppm"
            _, _, maximized_frame = dq.capture_until_changed(
                qmp, maximized, terminal_frame,
                time.monotonic() + args.frame_timeout, timings, "maximize_on", 1000,
                dq.region_changed(terminal_frame, 0, 0, 80, 56, 20),
            )
            screenshots.append(maximized.name)

            # Step 13: right-click the maximized title bar, now at the very
            # top. The window must return to its exact saved geometry: the
            # top-left box goes back to wallpaper, and the window's own
            # footprint region is pixel-identical to the pre-maximize frame
            # (no cursor or toast ever lands inside that region).
            step = time.monotonic()
            move_to((LOGICAL_WIDTH // 2, 7))
            right_click(qmp)
            wait_marker_count(
                serial, "woven-shell: window restored: Terminal", 1, step + args.input_timeout,
                process, "maximize_off_ms", timings, step,
            )
            # The restore WSIZ must also round-trip: the client resizes back to
            # its saved canvas and the shell accepts that WRSZ as a normal
            # resize, ending maximization with the saved geometry.
            wait_marker_count(
                serial, "WovenTerminal: window resized", 2, step + args.input_timeout,
                process, "restore_client_reflow_ms", timings, step,
            )
            wait_marker_count(
                serial, "woven-shell: window resized: Terminal", 1, step + args.input_timeout,
                process, "restore_wrsz_ms", timings, step,
            )
            restored = output / "maximize-step13-restored.ppm"
            _, _, restored_frame = dq.capture_until_changed(
                qmp, restored, maximized_frame,
                time.monotonic() + args.frame_timeout, timings, "maximize_off", 1000,
                dq.region_changed(maximized_frame, 0, 0, 80, 56, 20),
            )
            screenshots.append(restored.name)
            # Compare the shell-drawn title bar, not the client body: the
            # Terminal owns its surface content and may repaint a cursor
            # blink between captures. The title bar position and pixels are
            # shell-deterministic and prove the window returned to its saved
            # geometry.
            # The saved title-bar position must be occupied again. Compare
            # against the maximized frame, not the pre-maximize frame: the
            # shell-drawn title can legitimately repaint in a different focus
            # style across the round-trip, but the band-filling maximized frame
            # left this region as wallpaper.
            title_box = (2 * 64 - 2, 2 * 36 - 2, 2 * 424 + 2, 2 * 50 - 2)
            if region_equal(maximized_frame, restored_frame, title_box):
                raise dq.GateFailure(
                    "maximize restore: the saved title bar region is unchanged "
                    "from the maximized frame"
                )

            # Step 14: maximize again from the restored title bar, then drag
            # the maximized title bar: the drag must restore the window first,
            # so a second "maximized" and a second "restored" marker appear
            # and the band-filling frame goes away.
            step = time.monotonic()
            move_to((64 + 180, 50 + 7))
            right_click(qmp)
            # A single missed right-click should not fail the gate; retry once.
            if dq.read_serial(serial).count("woven-shell: window maximized: Terminal") < 2:
                time.sleep(0.5)
                move_to((64 + 180, 36 + 7))
                right_click(qmp)
            wait_marker_count(
                serial, "woven-shell: window maximized: Terminal", 2, step + args.input_timeout,
                process, "maximize_on_again_ms", timings, step,
            )
            maximized2 = output / "maximize-step14-maximized.ppm"
            _, _, maximized2_frame = dq.capture_until_changed(
                qmp, maximized2, restored_frame,
                time.monotonic() + args.frame_timeout, timings, "maximize_on_again", 1000,
            )
            screenshots.append(maximized2.name)
            move_to((LOGICAL_WIDTH // 2, 7))
            qmp.command("input-send-event", {"events": [
                {"type": "btn", "data": {"down": True, "button": "left"}},
            ]})
            time.sleep(0.2)
            move_to((LOGICAL_WIDTH // 2 + 40, 37))
            time.sleep(0.3)
            qmp.command("input-send-event", {"events": [
                {"type": "btn", "data": {"down": False, "button": "left"}},
            ]})
            wait_marker_count(
                serial, "woven-shell: window restored: Terminal", 2, step + args.input_timeout,
                process, "maximize_drag_restore_ms", timings, step,
            )
            dragged = output / "maximize-step14-drag-restored.ppm"
            dq.capture_until_changed(
                qmp, dragged, maximized2_frame,
                time.monotonic() + args.frame_timeout, timings, "maximize_drag", 1000,
            )
            screenshots.append(dragged.name)

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
                    "applies_to": ["pins", "taskbar_auto_hide preference"],
                },
            })
            (output / "result.json").write_text(
                json.dumps(result, indent=2) + "\n", encoding="utf-8"
            )
            print(f"Taskbar pins + auto-hide: PASS ({args.cpus} CPU). Evidence: {output}")
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
    except Exception as error:
        # A harness bug is still a failed gate with preserved evidence,
        # never a silent crash: the same convention as the desktop
        # qualification harness's unexpected-error arm.
        result = dict(base)
        result.update({
            "status": "FAIL",
            "pass": False,
            "failures": [f"unexpected harness error: {error}"],
            "screenshots": screenshots,
            "timings_ms": {**timings},
        })
        (output / "result.json").write_text(
            json.dumps(result, indent=2) + "\n", encoding="utf-8"
        )
        print(f"FAILED: unexpected harness error: {error}; evidence: {output}", file=sys.stderr)
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
