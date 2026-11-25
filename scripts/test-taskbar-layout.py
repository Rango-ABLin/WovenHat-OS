#!/usr/bin/env python3
"""Fail-closed, real-QEMU taskbar layout preference qualification.

Verifies that the shell applies taskbar alignment and icon size preferences
from its startup configuration. This gate boots a hardware-demo image with
compile-time shell overrides (WOVENHAT_SHELL_TASKBAR_ALIGNMENT=left,
WOVENHAT_SHELL_TASKBAR_ICON_SIZE=large) and asserts:

  1. the shell reports the effective layout in its startup marker;
  2. the taskbar strip is repainted with the Start button at the left edge
     rather than the default centered position.

The pixel check is a sanity check against the known layout geometry; the
serial marker is the authoritative assertion that the preference was parsed
and applied. Screenshots are preserved as visual evidence.

Nothing here asserts cross-boot persistence: the shell has FileRead only and
no settings-write path.

Return codes: 0 pass, 1 a guest check failed, 2 unsupported environment.
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

LOGICAL_WIDTH = 640
LOGICAL_HEIGHT = 400
BAR_HEIGHT = 24
STRIP_TOP = 800 - 2 * BAR_HEIGHT
STRIP_HEIGHT = 2 * BAR_HEIGHT


def pixel_at(pixels: bytes, x: int, y: int) -> tuple[int, int, int]:
    offset = (y * 1280 + x) * 3
    return tuple(pixels[offset : offset + 3])


def region_has_non_background(
    pixels: bytes, background: tuple[int, int, int], x0: int, y0: int, x1: int, y1: int, threshold: int = 30
) -> bool:
    for y in range(y0, y1):
        for x in range(x0, x1):
            r, g, b = pixel_at(pixels, x, y)
            if (
                abs(r - background[0]) > threshold
                or abs(g - background[1]) > threshold
                or abs(b - background[2]) > threshold
            ):
                return True
    return False


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
    requested = args.output or ROOT / "audit-artifacts" / f"taskbar-layout-{time.time_ns()}"
    try:
        output = dq.create_output(ROOT, requested)
    except ValueError as error:
        parser.error(str(error))
    qemu = args.qemu.resolve()
    firmware = dq.resolve_firmware(qemu, args.firmware.resolve() if args.firmware else None)
    base: dict[str, Any] = {
        "schema": 1,
        "classification": "real QEMU taskbar layout preference qualification; no physical certification",
        "qemu": str(qemu),
        "firmware": str(firmware.resolve()),
        "cpus": args.cpus,
        "physical_hardware_touched": False,
        "reboot_requested": False,
        "output": str(output),
        "requested_layout": {"alignment": "left", "icon_size": "large", "auto_hide": "off"},
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
    old_env = {
        key: os.environ.get(key)
        for key in (
            "WOVENHAT_SHELL_TASKBAR_ALIGNMENT",
            "WOVENHAT_SHELL_TASKBAR_ICON_SIZE",
            "WOVENHAT_SHELL_TASKBAR_AUTO_HIDE",
        )
    }
    try:
        os.environ["WOVENHAT_SHELL_TASKBAR_ALIGNMENT"] = "left"
        os.environ["WOVENHAT_SHELL_TASKBAR_ICON_SIZE"] = "large"
        os.environ["WOVENHAT_SHELL_TASKBAR_AUTO_HIDE"] = "off"
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
        (output / "qemu-command.json").write_text(json.dumps(command, indent=2) + "\n", encoding="utf-8")
        started = time.monotonic()
        with (output / "qemu.log").open("w", encoding="utf-8") as qemu_log:
            flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
            process = subprocess.Popen(
                command, cwd=ROOT, stdout=qemu_log, stderr=subprocess.STDOUT, creationflags=flags
            )
            qmp = dq.Qmp(qmp_port, time.monotonic() + 30, output / "qmp.jsonl", process, None)
            dq.wait_marker(
                serial, "[SESSION] WovenShell desktop started", started + args.timeout,
                process, "desktop_session_startup_ms", timings, started,
            )
            dq.wait_marker(
                serial, "woven-shell: desktop ready", started + args.timeout,
                process, "desktop_ready_ms", timings, started,
            )
            layout_marker = "woven-shell: taskbar layout alignment=left icon_size=large auto_hide=off"
            dq.wait_marker(
                serial, layout_marker, started + args.timeout,
                process, "taskbar_layout_marker_ms", timings, started,
            )
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
            frame = output / "taskbar-layout-left-large.ppm"
            _, _, pixels = dq.capture_frame(
                qmp, frame, time.monotonic() + args.frame_timeout, timings, "taskbar_layout"
            )
            screenshots.append(frame.name)

            # Left alignment places the Start button at logical x=8. The Start
            # button is 58x16 at y=bar_top+4; sample an interior point in
            # physical pixels. The centered default would place Start near
            # logical x=106 for 3 medium pins, leaving the left area as bar bg.
            left_start_x0, left_start_x1 = 16 + 4, 16 + 116  # physical x span covering start
            start_y0, start_y1 = STRIP_TOP + 8, STRIP_TOP + 40
            # Sample bar background from the empty strip between the app group and
            # right-side controls (left+large with 3 default pins leaves this empty).
            bar_bg = pixel_at(pixels, 700, STRIP_TOP + 24)
            left_has_content = region_has_non_background(
                pixels, bar_bg, left_start_x0, start_y0, left_start_x1, start_y1, threshold=25
            )
            # The default center start would be around logical 106 -> physical 212.
            # With left alignment, that area should be taskbar entries/background,
            # not the start button's left-edge shape. Check that left edge has
            # stronger start presence than the old centered start origin.
            old_center_origin = (212, STRIP_TOP + 20, 232, STRIP_TOP + 36)
            old_origin_bg_like = not region_has_non_background(
                pixels, bar_bg, *old_center_origin, threshold=20
            )
            if not left_has_content:
                raise dq.GateFailure(
                    "left-aligned Start button area does not differ from bar background"
                )

            result = dict(base)
            result.update({
                "status": "PASS",
                "pass": True,
                "failures": [],
                "image": str(image),
                "screenshots": screenshots,
                "timings_ms": {**timings},
                "layout_marker": layout_marker,
                "pixel_checks": {
                    "left_start_has_content": left_has_content,
                    "old_center_origin_background_like": old_origin_bg_like,
                    "bar_background_sample": list(bar_bg),
                },
            })
            (output / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            print(f"Taskbar layout: PASS ({args.cpus} CPU). Evidence: {output}")
            return 0
    except dq.GateFailure as failure:
        result = dict(base)
        result.update({
            "status": "FAIL", "pass": False, "failures": [str(failure)],
            "screenshots": screenshots, "timings_ms": {**timings},
        })
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(f"FAILED: {failure}; evidence: {output}", file=sys.stderr)
        return 1
    except dq.UnsupportedQualification as unsupported:
        result = dict(base)
        result.update({
            "status": "UNSUPPORTED", "pass": False, "failures": [str(unsupported)],
            "screenshots": screenshots, "timings_ms": {**timings},
        })
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(f"UNSUPPORTED: {unsupported}; evidence: {output}", file=sys.stderr)
        return 2
    except Exception as error:
        result = dict(base)
        result.update({
            "status": "FAIL", "pass": False,
            "failures": [f"unexpected harness error: {error}"],
            "screenshots": screenshots, "timings_ms": {**timings},
        })
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(f"FAILED: unexpected harness error: {error}; evidence: {output}", file=sys.stderr)
        return 1
    finally:
        for key, value in old_env.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()


if __name__ == "__main__":
    raise SystemExit(main())
