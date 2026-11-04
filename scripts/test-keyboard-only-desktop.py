"""Qualify the bounded keyboard-only desktop workflow in real QEMU.

The path signs in, opens the launcher, launches Calculator, enters and
evaluates an expression, then closes the focused window with Escape. No
pointer event is injected. This is a software/QEMU acceptance boundary, not
physical keyboard certification.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
HELPER_PATH = ROOT / "scripts" / "test-desktop-qualification.py"
SPEC = importlib.util.spec_from_file_location("desktop_qualification", HELPER_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("desktop qualification helper cannot be loaded")
HELPER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HELPER)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--qemu", type=Path, default=Path(HELPER.DEFAULT_QEMU))
    parser.add_argument("--firmware", type=Path)
    args = parser.parse_args()

    output = HELPER.create_output(ROOT, None)
    qemu = args.qemu.resolve()
    firmware = HELPER.resolve_firmware(qemu, args.firmware.resolve() if args.firmware else None)
    base = {
        "schema": 1,
        "classification": "real QEMU keyboard-only desktop qualification; no physical certification",
        "cpus": args.cpus,
        "qemu": str(qemu),
        "firmware": str(firmware),
        "output": str(output),
        "pointer_events_injected": False,
    }
    blockers = HELPER.preflight(qemu, firmware)
    if blockers:
        base.update({"status": "UNSUPPORTED", "pass": False, "blockers": blockers})
        HELPER.write_result(output, base)
        print(json.dumps(base, indent=2), file=sys.stderr)
        return 2

    image = None
    process = None
    qmp = None
    serial = output / "serial.log"
    timings = {}
    try:
        image = HELPER.build_hardware_demo(output)
        port = HELPER.free_port()
        command = [
            str(qemu), "-accel", "whpx", "-machine", "pc", "-m", "5G",
            "-smp", str(args.cpus), "-serial", f"file:{serial}", "-no-reboot",
            "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
            "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
            "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
            "-device", "virtio-blk-pci,drive=boot,bootindex=1",
            "-display", "none", "-vga", "std",
        ]
        (output / "qemu-command.json").write_text(json.dumps(command, indent=2) + "\n", encoding="utf-8")
        with (output / "qemu.log").open("w", encoding="utf-8") as qemu_log:
            flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
            process = subprocess.Popen(command, cwd=ROOT, stdout=qemu_log,
                                       stderr=subprocess.STDOUT, creationflags=flags)
            started = time.monotonic()
            qmp = HELPER.Qmp(port, started + 30, output / "qmp.jsonl", process)
            HELPER.wait_marker(serial, "[SESSION] WovenShell desktop started",
                               started + 180, process, "desktop_startup_ms", timings, started)
            HELPER.wait_marker(serial, "woven-shell: desktop ready",
                               started + 180, process, "desktop_ready_ms", timings, started)
            time.sleep(8.0)

            for key in ("1", "2", "3", "4", "ret"):
                qmp.key(key, 150)
            HELPER.wait_marker(serial, "woven-shell: signed in: ada",
                               time.monotonic() + 20, process, "sign_in_ms", timings, started)

            before = output / "before-launch.ppm"
            HELPER.capture_frame(qmp, before, time.monotonic() + 5, timings, "before_launch")
            qmp.key("f1", 150)
            qmp.key("ret", 150)
            HELPER.wait_marker(serial, "WovenCalculator: window opened",
                               time.monotonic() + 20, process, "calculator_open_ms", timings, started)

            calculator = output / "calculator.ppm"
            _, _, before_pixels = HELPER.capture_until_changed(
                qmp,
                calculator,
                HELPER.parse_ppm(before)[2],
                time.monotonic() + 10,
                timings,
                "calculator",
                1000,
            )
            for key in ("1", "2", "minus", "3", "ret"):
                qmp.key(key, 120)
            HELPER.wait_marker(serial, "WovenCalculator: input updated",
                               time.monotonic() + 10, process, "calculator_input_ms", timings, started)
            after = output / "calculator-evaluated.ppm"
            _, _, _ = HELPER.capture_until_changed(
                qmp,
                after,
                before_pixels,
                time.monotonic() + 10,
                timings,
                "calculator_evaluated",
                1000,
            )

            qmp.key("esc", 150)
            HELPER.wait_marker(serial, "woven-shell: keyboard close requested",
                               time.monotonic() + 10, process, "keyboard_close_ms", timings, started)
            HELPER.wait_marker(serial, "woven-shell: reaped WovenCalculator",
                               time.monotonic() + 10, process, "calculator_reaped_ms", timings, started)
            if HELPER.serial_has_fatal_evidence(HELPER.read_serial(serial)):
                raise HELPER.GateFailure("fatal kernel/security evidence found")
            qmp.command("quit")
            process.wait(timeout=10)
            if process.returncode != 0:
                raise HELPER.GateFailure(f"QEMU quit returned {process.returncode}")
        base.update({"status": "PASS", "pass": True, "timings_ms": timings})
        HELPER.write_result(output, base)
        print(f"Keyboard-only desktop qualification: PASS ({args.cpus} CPU). Evidence: {output}")
        return 0
    except Exception as error:  # noqa: BLE001 - preserve fail-closed evidence
        base.update({"status": "FAIL", "pass": False, "failures": [str(error)], "timings_ms": timings})
        HELPER.write_result(output, base)
        print(f"FAILED: {error}; evidence: {output}", file=sys.stderr)
        return 1
    finally:
        if qmp is not None:
            try:
                qmp.close()
            except OSError:
                pass
        HELPER.reap_process(process)


if __name__ == "__main__":
    raise SystemExit(main())
