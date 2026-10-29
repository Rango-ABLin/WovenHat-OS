#!/usr/bin/env python3
"""Fail-closed, real-QEMU desktop/input/performance qualification.

This gate owns the QEMU run instead of treating a host model as evidence.  It
builds the existing hardware-demo desktop, boots it with an explicit OVMF
CODE image, injects keyboard and PS/2 mouse events through QMP, captures the
guest framebuffer, and records phase timings plus all serial/QEMU evidence.

Return codes:
  0  all requested guest checks passed;
  1  QEMU ran but a guest check failed;
  2  the gate is unsupported in this environment (missing QEMU/OVMF or a
     build prerequisite).  Unsupported is never reported as PASS.

No physical device, host display, firmware variable store, or disk is used.
The guest boot image is passed read-only to QEMU and every evidence directory
is unique; an existing output directory is never overwritten.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_QEMU = shutil.which("qemu-system-x86_64") or (
    r"C:\Program Files\qemu\qemu-system-x86_64.exe"
)
TEST_PUBLIC_KEY = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
APP_IDS = (
    "woven-calculator",
    "woven-terminal",
    "woven-files",
    "woven-monitor",
    "woven-settings",
    "woven-security",
    "woven-editor",
    "woven-images",
    "woven-update",
    "woven-store",
    "woven-installer",
)


class UnsupportedQualification(RuntimeError):
    """The requested real-QEMU qualification cannot run here."""


class GateFailure(RuntimeError):
    """A real-QEMU gate ran and produced failed evidence."""


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(("127.0.0.1", 0))
        return int(probe.getsockname()[1])


def resolve_firmware(qemu: Path, supplied: Path | None) -> Path:
    if supplied is not None:
        return supplied
    return qemu.parent / "share" / "edk2-x86_64-code.fd"


def preflight(qemu: Path, firmware: Path) -> list[str]:
    blockers: list[str] = []
    if not qemu.is_file():
        blockers.append(f"QEMU executable is unavailable: {qemu}")
    if not firmware.is_file():
        blockers.append(f"OVMF CODE firmware is unavailable: {firmware}")
    if not shutil.which("cargo"):
        blockers.append("cargo is unavailable; the current guest image cannot be built")
    return blockers


def create_output(root: Path, requested: Path | None) -> Path:
    output = (requested or root / "audit-artifacts" / f"desktop-qualification-{time.time_ns()}").resolve()
    if output.exists():
        raise ValueError(f"refusing to overwrite existing evidence directory: {output}")
    output.mkdir(parents=True, exist_ok=False)
    return output


def _skip_space_and_comments(data: bytes, index: int) -> int:
    while True:
        while index < len(data) and data[index:index + 1].isspace():
            index += 1
        if index < len(data) and data[index:index + 1] == b"#":
            newline = data.find(b"\n", index)
            index = len(data) if newline < 0 else newline + 1
            continue
        return index


def parse_ppm(path: Path) -> tuple[int, int, bytes]:
    data = path.read_bytes()
    tokens: list[bytes] = []
    index = 0
    while len(tokens) < 4:
        index = _skip_space_and_comments(data, index)
        start = index
        while index < len(data) and not data[index:index + 1].isspace() and data[index:index + 1] != b"#":
            index += 1
        if start == index:
            raise ValueError(f"incomplete PPM header in {path}")
        tokens.append(data[start:index])
    if tokens[0] != b"P6" or tokens[3] != b"255":
        raise ValueError(f"unsupported PPM header in {path}: {tokens!r}")
    try:
        width, height = int(tokens[1]), int(tokens[2])
    except ValueError as error:
        raise ValueError(f"non-numeric PPM dimensions in {path}") from error
    if width <= 0 or height <= 0:
        raise ValueError(f"invalid PPM dimensions in {path}: {width}x{height}")
    # The max-value token is followed by one required whitespace delimiter.
    # Do not skip an arbitrary run here: the first RGB byte is binary payload
    # and may itself be classified as ASCII whitespace (for example 0x0c).
    if index >= len(data) or not data[index:index + 1].isspace():
        raise ValueError(f"missing PPM payload delimiter in {path}")
    index += 1
    expected = width * height * 3
    pixels = data[index:]
    if len(pixels) != expected:
        raise ValueError(
            f"PPM payload length mismatch in {path}: expected {expected}, got {len(pixels)}"
        )
    if len(set(pixels[i:i + 3] for i in range(0, len(pixels), 3))) < 2:
        raise ValueError(f"uniform framebuffer in {path}")
    return width, height, pixels


def changed_pixels(left: bytes, right: bytes) -> int:
    if len(left) != len(right) or len(left) % 3:
        raise ValueError("framebuffer payloads have incompatible sizes")
    return sum(left[offset:offset + 3] != right[offset:offset + 3]
               for offset in range(0, len(left), 3))


def serial_has_fatal_evidence(log: str) -> bool:
    lowered = log.lower()
    return any(marker in lowered for marker in (
        "kernel panic", "panicked at", "security violation", "security: failed",
    ))


def timing_within(timing: dict[str, float], limits: dict[str, float]) -> list[str]:
    failures = []
    for name, limit in limits.items():
        value = timing.get(name)
        if value is not None and value > limit:
            failures.append(f"{name}={value:.1f}ms exceeds limit {limit:.1f}ms")
    return failures


class Qmp:
    def __init__(self, port: int, deadline: float, trace: Path | None = None,
                 process: subprocess.Popen[Any] | None = None):
        while True:
            try:
                self.sock = socket.create_connection(("127.0.0.1", port), timeout=2)
                break
            except OSError:
                if process is not None and process.poll() is not None:
                    raise RuntimeError(f"QEMU exited before QMP opened (status {process.returncode})")
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.1)
        self.file = self.sock.makefile("rwb")
        self.trace = trace
        self._read_reply()
        self.command("qmp_capabilities")

    def _read_reply(self) -> dict[str, Any]:
        line = self.file.readline()
        if not line:
            raise EOFError("QMP disconnected")
        reply = json.loads(line.decode("utf-8"))
        if self.trace is not None:
            with self.trace.open("a", encoding="utf-8") as trace:
                trace.write(json.dumps({"direction": "in", "message": reply}) + "\n")
        return reply

    def command(self, name: str, arguments: dict[str, Any] | None = None) -> Any:
        request: dict[str, Any] = {"execute": name}
        if arguments is not None:
            request["arguments"] = arguments
        if self.trace is not None:
            with self.trace.open("a", encoding="utf-8") as trace:
                trace.write(json.dumps({"direction": "out", "message": request}) + "\n")
        self.file.write((json.dumps(request) + "\n").encode("utf-8"))
        self.file.flush()
        while True:
            reply = self._read_reply()
            if "error" in reply:
                raise RuntimeError(f"QMP {name} failed: {reply['error']}")
            if "return" in reply:
                return reply["return"]

    def key(self, code: str, hold_ms: int = 100) -> None:
        # `send-key` is a monitor convenience command.  Explicit key events
        # exercise the same input device path as an interactive QEMU window
        # and remain reliable when the guest is headless (`-display none`).
        self.command("input-send-event", {"events": [
            {"type": "key", "data": {"down": True, "key": {"type": "qcode", "data": code}}},
            {"type": "key", "data": {"down": False, "key": {"type": "qcode", "data": code}}},
        ]})
        if hold_ms > 0:
            time.sleep(hold_ms / 1000.0)

    def relative_mouse(self, dx: int, dy: int) -> None:
        events = []
        if dx:
            events.append({"type": "rel", "data": {"axis": "x", "value": dx}})
        if dy:
            events.append({"type": "rel", "data": {"axis": "y", "value": dy}})
        if events:
            self.command("input-send-event", {"events": events})

    def click(self) -> None:
        self.command("input-send-event", {"events": [
            {"type": "btn", "data": {"down": True, "button": "left"}},
        ]})
        time.sleep(0.08)
        self.command("input-send-event", {"events": [
            {"type": "btn", "data": {"down": False, "button": "left"}},
        ]})

    def screendump(self, path: Path) -> None:
        self.command("screendump", {"filename": str(path.resolve())})

    def close(self) -> None:
        try:
            self.file.close()
        finally:
            self.sock.close()


def read_serial(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""


def wait_marker(
    serial: Path,
    marker: str,
    deadline: float,
    process: subprocess.Popen[Any],
    phase: str,
    timings: dict[str, float],
    started: float,
) -> None:
    while time.monotonic() < deadline:
        if marker in read_serial(serial):
            timings[phase] = (time.monotonic() - started) * 1000.0
            return
        if process.poll() is not None:
            raise GateFailure(f"QEMU exited before marker {marker!r} for {phase}")
        time.sleep(0.05)
    raise GateFailure(f"timed out waiting for marker {marker!r} for {phase}")


def capture_frame(qmp: Qmp, path: Path, deadline: float, timings: dict[str, float], label: str) -> tuple[int, int, bytes]:
    started = time.monotonic()
    qmp.screendump(path)
    while time.monotonic() < deadline:
        if path.is_file() and path.stat().st_size:
            try:
                frame = parse_ppm(path)
            except (OSError, ValueError):
                time.sleep(0.05)
                continue
            timings[f"frame_{label}_ms"] = (time.monotonic() - started) * 1000.0
            return frame
        time.sleep(0.05)
    raise GateFailure(f"timed out capturing valid framebuffer {label!r}")


def capture_until_changed(
    qmp: Qmp,
    path: Path,
    reference: bytes,
    deadline: float,
    timings: dict[str, float],
    label: str,
    minimum_changed: int,
) -> tuple[int, int, bytes]:
    """Retain real QMP scanouts until a repaint is visible or fail closed."""
    started = time.monotonic()
    attempt = 0
    while time.monotonic() < deadline:
        attempt_path = path.with_name(f"{path.stem}.attempt-{attempt}{path.suffix}")
        attempt += 1
        qmp.screendump(attempt_path)
        try:
            width, height, pixels = parse_ppm(attempt_path)
        except (OSError, ValueError):
            time.sleep(0.1)
            continue
        if len(reference) == len(pixels) and changed_pixels(reference, pixels) >= minimum_changed:
            shutil.copyfile(attempt_path, path)
            timings[f"frame_{label}_ms"] = (time.monotonic() - started) * 1000.0
            return width, height, pixels
        time.sleep(0.1)
    raise GateFailure(f"timed out waiting for a materially changed framebuffer {label!r}")


def build_hardware_demo(output: Path) -> Path:
    app_dir = ROOT / "apps" / "woven-shell"
    app_env = dict(os.environ)
    app_env.pop("CARGO_BUILD_BUILD_DIR", None)
    app_env["CARGO_TARGET_DIR"] = str(app_dir / "target")
    shell_build = subprocess.run(
        ["cargo", "build", "--release"], cwd=app_dir, env=app_env,
        capture_output=True, text=True,
    )
    (output / "woven-shell-build.log").write_text(shell_build.stdout + shell_build.stderr, encoding="utf-8")
    if shell_build.returncode:
        raise UnsupportedQualification("woven-shell release build failed; inspect woven-shell-build.log")
    shell_image = app_dir / "target" / "x86_64-unknown-none" / "release" / "wovenhat-woven-shell"
    if not shell_image.is_file():
        raise UnsupportedQualification(f"woven-shell image was not produced: {shell_image}")

    package_env = dict(os.environ)
    package_env.setdefault("WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX", TEST_PUBLIC_KEY)
    package = subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "package-system-apps.py"), "--version", "0.8.0"],
        cwd=ROOT, env=package_env, capture_output=True, text=True,
    )
    (output / "system-app-package.log").write_text(package.stdout + package.stderr, encoding="utf-8")
    if package.returncode:
        raise UnsupportedQualification("system-app packaging failed; inspect system-app-package.log")
    catalog = ROOT / "target" / "system-apps" / "0.8.0"
    manifest_path = catalog / "manifest.json"
    if not manifest_path.is_file():
        raise UnsupportedQualification(f"system-app manifest was not produced: {manifest_path}")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    packages = {item["id"]: catalog / item["package"] for item in manifest.get("apps", [])}
    missing = [app_id for app_id in APP_IDS if not packages.get(app_id, Path()).is_file()]
    if missing:
        raise UnsupportedQualification(f"signed desktop packages are missing: {', '.join(missing)}")
    service = subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "build-package-service.py")],
        cwd=ROOT, env=package_env, capture_output=True, text=True,
    )
    (output / "package-service-build.log").write_text(service.stdout + service.stderr, encoding="utf-8")
    if service.returncode or not service.stdout.strip():
        raise UnsupportedQualification("package service build failed; inspect package-service-build.log")

    env = dict(os.environ)
    env["WOVENHAT_SHELL_IMAGE"] = str(shell_image)
    env["WOVENHAT_PACKAGE_IMAGE"] = service.stdout.strip().splitlines()[-1]
    search_service = subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "build-search-service.py")],
        cwd=ROOT, env=package_env, capture_output=True, text=True,
    )
    (output / "search-service-build.log").write_text(
        search_service.stdout + search_service.stderr, encoding="utf-8"
    )
    if search_service.returncode or not search_service.stdout.strip():
        raise UnsupportedQualification("search service build failed; inspect search-service-build.log")
    env["WOVENHAT_SEARCH_IMAGE"] = search_service.stdout.strip().splitlines()[-1]
    env["WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX"] = TEST_PUBLIC_KEY
    variables = {
        "woven-calculator": "WOVENHAT_CALCULATOR_IMAGE",
        "woven-terminal": "WOVENHAT_TERMINAL_IMAGE",
        "woven-files": "WOVENHAT_FILES_IMAGE",
        "woven-monitor": "WOVENHAT_MONITOR_IMAGE",
        "woven-settings": "WOVENHAT_SETTINGS_IMAGE",
        "woven-security": "WOVENHAT_SECURITY_IMAGE",
        "woven-editor": "WOVENHAT_EDITOR_IMAGE",
        "woven-images": "WOVENHAT_IMAGES_IMAGE",
        "woven-update": "WOVENHAT_UPDATE_IMAGE",
        "woven-store": "WOVENHAT_STORE_IMAGE",
        "woven-installer": "WOVENHAT_INSTALLER_IMAGE",
    }
    for app_id, variable in variables.items():
        env[variable] = str(packages[app_id])
    env["CARGO_TARGET_DIR"] = str(ROOT / "target")
    env["CARGO_BUILD_BUILD_DIR"] = str(ROOT / ".cargo-build")
    build = subprocess.run(
        ["cargo", "run", "--quiet", "--release", "--features", "hardware-demo",
         "--target-dir", str(ROOT / "target"), "--", "--print-image"],
        cwd=ROOT, env=env, capture_output=True, text=True,
    )
    (output / "kernel-build.log").write_text(build.stdout + build.stderr, encoding="utf-8")
    if build.returncode:
        raise UnsupportedQualification("hardware-demo kernel build failed; inspect kernel-build.log")
    image_text = build.stdout.strip()
    image = Path(image_text.splitlines()[-1]) if image_text else Path()
    if not image.is_file():
        raise UnsupportedQualification(f"hardware-demo boot image was not produced: {image}")
    return image


def reap_process(process: subprocess.Popen[Any] | None) -> None:
    """Reap only this gate's QEMU child, including a stubborn timeout path."""
    if process is None:
        return
    if process.poll() is None:
        process.kill()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        # A second wait is deliberately bounded; this is still only the child
        # created by this gate, never an unrelated QEMU process.
        process.kill()
        process.wait(timeout=10)


def run_guest(args: argparse.Namespace, output: Path, qemu: Path, firmware: Path) -> dict[str, Any]:
    image = build_hardware_demo(output)
    serial = output / "serial.log"
    qmp_port = free_port()
    command = [
        str(qemu), "-accel", "whpx" if args.accelerator == "whpx" else "tcg,tb-size=128",
        "-machine", "pc", "-m", args.memory,
        "-smp", str(args.cpus), "-display", "none", "-serial", f"file:{serial}",
        "-no-reboot", "-qmp", f"tcp:127.0.0.1:{qmp_port},server=on,wait=off",
        "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
        "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
        "-device", "virtio-blk-pci,drive=boot,bootindex=1", "-vga", "std",
    ]
    (output / "qemu-command.json").write_text(json.dumps(command, indent=2) + "\n", encoding="utf-8")
    timings: dict[str, float] = {}
    screenshots: list[str] = []
    process: subprocess.Popen[Any] | None = None
    qmp: Qmp | None = None
    started = time.monotonic()
    try:
        with (output / "qemu.log").open("w", encoding="utf-8") as qemu_log:
            flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
            process = subprocess.Popen(command, cwd=ROOT, stdout=qemu_log, stderr=subprocess.STDOUT, creationflags=flags)
            qmp = Qmp(qmp_port, time.monotonic() + 30, output / "qmp.jsonl", process)
            wait_marker(serial, "[SESSION] WovenShell desktop started", started + args.timeout, process,
                        "desktop_session_startup_ms", timings, started)
            wait_marker(serial, "woven-shell: desktop ready", started + args.timeout, process,
                        "desktop_ready_ms", timings, started)

            # The shell publishes its desktop marker before the first frame
            # and input-focus lease have necessarily reached the lock screen.
            # Match the established hardware-demo harness's handoff settling
            # window and retain every attempt in the serial/QEMU evidence.
            # The desktop marker precedes the final compositor focus handoff;
            # allow the guest a full settling interval before injecting the
            # PIN, especially under host CPU contention.
            time.sleep(8.0)
            input_started = time.monotonic()
            for attempt in range(3):
                for key in ("1", "2", "3", "4", "ret"):
                    qmp.key(key, 150)
                    time.sleep(0.4)
                if "woven-shell: signed in: ada" in read_serial(serial):
                    break
                time.sleep(0.5)
            wait_marker(serial, "woven-shell: signed in: ada", time.monotonic() + args.input_timeout,
                        process, "sign_in_input_ms", timings, input_started)
            time.sleep(2.0)

            baseline = output / "desktop-signed-in.ppm"
            capture_frame(qmp, baseline, time.monotonic() + args.frame_timeout, timings, "desktop")
            screenshots.append(baseline.name)

            launch_started = time.monotonic()
            for key in ("f1", "f", "i", "l", "e", "s", "ret"):
                qmp.key(key, 90)
                time.sleep(0.12)
            wait_marker(serial, "WovenFiles: window opened", launch_started + args.input_timeout,
                        process, "files_window_open_ms", timings, launch_started)
            wait_marker(serial, "woven-shell: window opened: Files", launch_started + args.input_timeout,
                        process, "files_window_composed_ms", timings, launch_started)
            time.sleep(2.0)
            files_frame = output / "files-window-open.ppm"
            _, _, desktop_pixels = parse_ppm(baseline)
            files_width, files_height, files_pixels = capture_until_changed(
                qmp, files_frame, desktop_pixels, time.monotonic() + args.frame_timeout,
                timings, "files_window", 1000
            )
            screenshots.append(files_frame.name)
            baseline_width, baseline_height, _ = parse_ppm(baseline)
            if (files_width, files_height) != (baseline_width, baseline_height):
                raise GateFailure("Files window did not materially change the presented desktop")

            pointer_started = time.monotonic()
            # QMP reports PS/2 motion in the guest's logical compositor
            # coordinates.  The 1280x800 framebuffer is presented at 2x, so
            # do not use screenshot-pixel deltas here.  The shell starts at
            # the logical 640x400 centre; move to (260,246), which is inside
            # WovenFiles' refresh button at client origin (40,40).
            qmp.relative_mouse(-60, 46)
            time.sleep(0.3)
            time.sleep(0.5)
            qmp.click()
            wait_marker(serial, "WovenFiles: pointer refreshed", pointer_started + args.input_timeout,
                        process, "mouse_click_input_ms", timings, pointer_started)
            time.sleep(1.0)
            pointer_frame = output / "files-pointer-click.ppm"
            _, _, before_pointer = parse_ppm(files_frame)
            _, _, after_pointer = capture_frame(
                qmp, pointer_frame, time.monotonic() + args.frame_timeout, timings, "pointer_click"
            )
            screenshots.append(pointer_frame.name)
            if changed_pixels(before_pointer, after_pointer) < 20:
                raise GateFailure("mouse click did not change the client framebuffer")

            # An aborted press must not become a click when the workspace
            # hides its owner. Keep the physical pointer inside Refresh.
            refreshes = read_serial(serial).count("WovenFiles: pointer refreshed")
            cancel_started = time.monotonic()
            qmp.command("input-send-event", {"events": [
                {"type": "btn", "data": {"down": True, "button": "left"}},
            ]})
            time.sleep(0.3)
            qmp.key("f3", 90)
            wait_marker(serial, "WovenFiles: pointer cancelled",
                        cancel_started + args.input_timeout, process,
                        "files_pointer_cancel_ms", timings, cancel_started)
            for _ in range(3):
                qmp.key("f3", 90)
                time.sleep(0.2)
            wait_marker(serial, "woven-shell: workspace 1",
                        cancel_started + args.input_timeout, process,
                        "files_workspace_return_ms", timings, cancel_started)
            qmp.command("input-send-event", {"events": [
                {"type": "btn", "data": {"down": False, "button": "left"}},
            ]})
            time.sleep(1.0)
            if read_serial(serial).count("WovenFiles: pointer refreshed") != refreshes:
                raise GateFailure("workspace cancellation activated the Refresh button")

            keyboard_refresh_started = time.monotonic()
            qmp.key("ret", 90)
            wait_marker(serial, "WovenFiles: keyboard refreshed",
                        keyboard_refresh_started + args.input_timeout, process,
                        "files_keyboard_refresh_ms", timings, keyboard_refresh_started)
            # The same-client FIFO has now consumed cancellation and release.
            if read_serial(serial).count("WovenFiles: pointer refreshed") != refreshes:
                raise GateFailure("delayed workspace cancellation activated Refresh")
            time.sleep(1.0)
            keyboard_frame = output / "files-keyboard-refresh.ppm"
            _, _, after_keyboard = capture_frame(
                qmp, keyboard_frame, time.monotonic() + args.frame_timeout,
                timings, "keyboard_refresh"
            )
            screenshots.append(keyboard_frame.name)
            if changed_pixels(after_pointer, after_keyboard) < 20:
                raise GateFailure("keyboard refresh did not change the client framebuffer")

            close_files_started = time.monotonic()
            # The Files window opens at (40,40), with a 320px client and a
            # ten-pixel close box at the right of its 14px title bar.  The
            # pointer is at the refresh control (260,246) after the click.
            # Files close box: global (40 + 320 - 12, 40 - 14 + 2).
            qmp.relative_mouse(88, -218)
            qmp.click()
            wait_marker(serial, "woven-shell: window closed: Files", close_files_started + args.input_timeout,
                        process, "files_window_close_ms", timings, close_files_started)

            settings_started = time.monotonic()
            for key in ("f1", "s", "e", "t", "t", "i", "n", "g", "s", "ret"):
                qmp.key(key, 90)
                time.sleep(0.12)
            wait_marker(serial, "woven-shell: window opened: Settings",
                        settings_started + args.input_timeout, process,
                        "settings_window_open_ms", timings, settings_started)
            time.sleep(1.0)
            settings_before = output / "settings-window-open.ppm"
            _, _, before_settings_key = capture_frame(
                qmp, settings_before, time.monotonic() + args.frame_timeout,
                timings, "settings_window"
            )
            screenshots.append(settings_before.name)
            settings_key_started = time.monotonic()
            qmp.key("ret", 90)
            wait_marker(serial, "WovenSettings: keyboard refreshed",
                        settings_key_started + args.input_timeout, process,
                        "settings_keyboard_refresh_ms", timings, settings_key_started)
            time.sleep(1.0)
            settings_after = output / "settings-keyboard-refresh.ppm"
            # A generic delta can be an unfinished opening repaint. Require
            # the complete gold focus strip on the actual Refresh control.
            # Logical window (64,64), button (250,190), width 90, scale 2.
            focus_started = time.monotonic()
            focus_deadline = focus_started + args.frame_timeout
            attempt = 0
            while time.monotonic() < focus_deadline:
                candidate = settings_after.with_name(f"settings-focus.attempt-{attempt}.ppm")
                attempt += 1
                width, height, after_settings_key = capture_frame(
                    qmp, candidate, focus_deadline, timings, "settings_focus_sample"
                )
                if width == 1280 and height == 800 and all(
                    after_settings_key[(y * width + 628) * 3:(y * width + 808) * 3]
                    == bytes((255, 205, 96)) * 180
                    for y in range(508, 510)
                ):
                    shutil.copyfile(candidate, settings_after)
                    timings["frame_settings_keyboard_refresh_ms"] = (
                        time.monotonic() - focus_started
                    ) * 1000.0
                    break
                time.sleep(0.1)
            else:
                raise GateFailure("Settings keyboard focus strip was not presented")
            screenshots.append(settings_after.name)
            if changed_pixels(before_settings_key, after_settings_key) < 20:
                raise GateFailure("Settings keyboard refresh did not change the client framebuffer")
            # The shell cascades each newly opened window by (24,24), even
            # after the earlier window closes. Settings opens at (64,64).
            qmp.relative_mouse(69, 29)
            qmp.click()
            settings_close_started = time.monotonic()
            wait_marker(serial, "woven-shell: window closed: Settings",
                        settings_close_started + args.input_timeout, process,
                        "settings_window_close_ms", timings, settings_close_started)
            qmp.relative_mouse(-69, -29)

            terminal_started = time.monotonic()
            for key in ("f1", "t", "e", "r", "m", "i", "n", "a", "l", "ret"):
                qmp.key(key, 90)
                time.sleep(0.12)
            wait_marker(serial, "WovenTerminal: window opened", terminal_started + args.input_timeout,
                        process, "terminal_window_open_ms", timings, terminal_started)
            # The client emits its marker immediately after sending WOPN.  Do
            # not inject focus or keyboard input until WovenShell has consumed
            # that IPC message and composed the window; otherwise a valid
            # input sequence can land in the pre-window desktop state.
            wait_marker(serial, "woven-shell: window opened: Terminal",
                        terminal_started + args.input_timeout, process,
                        "terminal_window_composed_ms", timings, terminal_started)
            terminal_frame = output / "terminal-window-open.ppm"
            capture_frame(qmp, terminal_frame, time.monotonic() + args.frame_timeout, timings, "terminal_window")
            screenshots.append(terminal_frame.name)

            keyboard_started = time.monotonic()
            # Put focus unambiguously in the terminal client before sending
            # the keyboard event; the shell routes keys to its focused window.
            # From the Files close position (348,28), move into the terminal
            # body at (100,100).
            qmp.relative_mouse(-248, 72)
            qmp.click()
            time.sleep(0.3)
            qmp.key("h", 100)
            # Terminal GUI updates are intentionally silent on serial; the
            # framebuffer delta below is the acceptance signal for keyboard
            # delivery. Retain a bounded settle time for the guest IPC turn.
            wait_marker(serial, "WovenTerminal: input updated", keyboard_started + args.input_timeout,
                        process, "keyboard_input_delivery_ms", timings, keyboard_started)
            time.sleep(0.5)
            timings["keyboard_input_ms"] = (time.monotonic() - keyboard_started) * 1000.0
            time.sleep(1.0)
            keyboard_frame = output / "terminal-keyboard-input.ppm"
            _, _, terminal_before = parse_ppm(terminal_frame)
            _, _, terminal_after = capture_frame(
                qmp, keyboard_frame, time.monotonic() + args.frame_timeout, timings, "keyboard_input"
            )
            screenshots.append(keyboard_frame.name)
            if changed_pixels(terminal_before, terminal_after) < 20:
                raise GateFailure("keyboard input did not change the terminal framebuffer")

            close_terminal_started = time.monotonic()
            # Files is the first cascade window at (40,40); CASCADE_STEP is
            # 24; after Settings, Terminal is the third at (88,88), width 360.
            # Window::close_area() is inset by two pixels from the right edge
            # and starts two pixels into the 14-pixel title bar, so its
            # center is (441,81). The focus click leaves the pointer at
            # (100,100).
            qmp.relative_mouse(341, -19)
            qmp.click()
            # Keep the click qualification tolerant of one-pixel rounding in
            # the PS/2 logical-coordinate conversion while still requiring
            # the shell's real close request below.
            for dx, dy in ((-4, 0), (4, 0), (0, -4), (0, 4)):
                time.sleep(0.15)
                if "woven-shell: window closed: Terminal" in read_serial(serial):
                    break
                qmp.relative_mouse(dx, dy)
                qmp.click()
            wait_marker(serial, "woven-shell: window closed: Terminal", close_terminal_started + args.input_timeout,
                        process, "terminal_window_close_ms", timings, close_terminal_started)
            time.sleep(1.0)
            closed_frame = output / "desktop-after-window-close.ppm"
            _, _, closed_pixels = capture_frame(
                qmp, closed_frame, time.monotonic() + args.frame_timeout, timings, "window_close"
            )
            screenshots.append(closed_frame.name)
            if changed_pixels(terminal_after, closed_pixels) < 1000:
                raise GateFailure("closing the terminal did not materially change the desktop framebuffer")

            serial_log = read_serial(serial)
            if serial_has_fatal_evidence(serial_log):
                raise GateFailure("fatal kernel/security evidence found in serial.log")
            if process.poll() is not None:
                raise GateFailure("QEMU exited before evidence collection completed")
            qmp.command("quit")
            process.wait(timeout=10)
            if process.returncode != 0:
                raise GateFailure(f"QEMU quit returned {process.returncode}")
    except (OSError, EOFError, ValueError, RuntimeError, GateFailure) as error:
        raise GateFailure(str(error)) from error
    finally:
        if qmp is not None:
            try:
                qmp.close()
            except OSError:
                pass
        reap_process(process)
    return {"timings_ms": timings, "screenshots": screenshots, "cpus": args.cpus}


def write_result(output: Path, result: dict[str, Any]) -> None:
    (output / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", type=Path, default=Path(DEFAULT_QEMU))
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--accelerator", choices=("tcg", "whpx"), default="tcg",
                        help="QEMU accelerator; assertions and timing limits are identical")
    parser.add_argument("--memory", default="5G", help="guest RAM (default: 5G configured to leave 4 GiB usable after firmware reservations)")
    parser.add_argument("--timeout", type=float, default=180.0)
    # TCG/QEMU startup and PS/2 input can cross 15 seconds on a loaded host;
    # keep the bound finite while matching the accepted 1/2/4-CPU evidence.
    parser.add_argument("--input-timeout", type=float, default=20.0)
    parser.add_argument("--frame-timeout", type=float, default=5.0)
    parser.add_argument("--max-startup-ms", type=float, default=60_000.0)
    parser.add_argument("--max-input-ms", type=float, default=20_000.0)
    parser.add_argument("--max-frame-ms", type=float, default=5_000.0)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    if args.timeout <= 0 or args.input_timeout <= 0 or args.frame_timeout <= 0:
        parser.error("timeouts must be positive")
    try:
        output = create_output(ROOT, args.output)
    except ValueError as error:
        parser.error(str(error))
    qemu = args.qemu.resolve()
    firmware = resolve_firmware(qemu, args.firmware.resolve() if args.firmware else None)
    base: dict[str, Any] = {
        "schema": 1,
        "classification": "real QEMU desktop/input/performance qualification; no physical certification",
        "qemu": str(qemu),
        "firmware": str(firmware.resolve()),
        "cpus": args.cpus,
        "accelerator": args.accelerator,
        "physical_hardware_touched": False,
        "reboot_requested": False,
        "output": str(output),
    }
    blockers = preflight(qemu, firmware)
    if blockers:
        base.update({"status": "UNSUPPORTED", "pass": False, "unsupported_blockers": blockers})
        write_result(output, base)
        print(json.dumps(base, indent=2), file=sys.stderr)
        return 2
    try:
        evidence = run_guest(args, output, qemu, firmware)
        limits = {
            "desktop_session_startup_ms": args.max_startup_ms,
            "desktop_ready_ms": args.max_startup_ms,
            "sign_in_input_ms": args.max_input_ms,
            "keyboard_input_ms": args.max_input_ms,
            "mouse_click_input_ms": args.max_input_ms,
            "files_pointer_cancel_ms": args.max_input_ms,
            "files_workspace_return_ms": args.max_input_ms,
            "files_keyboard_refresh_ms": args.max_input_ms,
            "settings_keyboard_refresh_ms": args.max_input_ms,
        }
        timing_failures = timing_within(evidence["timings_ms"], limits)
        frame_failures = [
            f"{name}={value:.1f}ms exceeds limit {args.max_frame_ms:.1f}ms"
            for name, value in evidence["timings_ms"].items()
            if name.startswith("frame_") and value > args.max_frame_ms
        ]
        failures = timing_failures + frame_failures
        base.update(evidence)
        base.update({"status": "FAIL" if failures else "PASS", "pass": not failures, "failures": failures})
        write_result(output, base)
        if failures:
            print("; ".join(failures), file=sys.stderr)
            return 1
        print(f"Desktop qualification: PASS ({args.cpus} CPU). Evidence: {output}")
        return 0
    except UnsupportedQualification as error:
        base.update({"status": "UNSUPPORTED", "pass": False, "unsupported_blockers": [str(error)]})
        write_result(output, base)
        print(f"UNSUPPORTED: {error}; evidence: {output}", file=sys.stderr)
        return 2
    except GateFailure as error:
        base.update({
            "status": "FAIL", "pass": False, "failures": [str(error)],
            "serial": "serial.log", "screenshots": sorted(path.name for path in output.glob("*.ppm")),
        })
        write_result(output, base)
        print(f"FAILED: {error}; evidence: {output}", file=sys.stderr)
        return 1
    except Exception as error:  # noqa: BLE001 - unexpected defects must fail closed with evidence
        base.update({
            "status": "FAIL", "pass": False, "failures": [f"unexpected harness error: {error}"],
            "serial": "serial.log", "screenshots": sorted(path.name for path in output.glob("*.ppm")),
        })
        write_result(output, base)
        print(f"FAILED: unexpected harness error: {error}; evidence: {output}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
