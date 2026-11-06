#!/usr/bin/env python3
"""Estimate physical input-to-visible-display latency with a webcam.

Point a webcam at the monitor and use a key that visibly changes the WovenHat
desktop. The host key-down timestamp is paired with the first camera frame
whose selected screen ROI changes beyond the threshold. This is a practical
webcam estimate, not photodiode-grade certification; camera exposure and frame
period are included in the uncertainty.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import statistics
import threading
import time

import cv2
from pynput import keyboard


def parse_roi(value: str) -> tuple[int, int, int, int]:
    try:
        x, y, width, height = (int(part) for part in value.split(","))
    except ValueError as exc:
        raise argparse.ArgumentTypeError("ROI must be x,y,width,height") from exc
    if width <= 0 or height <= 0 or x < 0 or y < 0:
        raise argparse.ArgumentTypeError("ROI dimensions must be positive")
    return x, y, width, height


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--camera-index", type=int, default=0)
    parser.add_argument("--roi", type=parse_roi, required=True,
                        help="camera ROI containing the monitor: x,y,width,height")
    parser.add_argument("--samples", type=int, default=10)
    parser.add_argument("--threshold", type=float, default=18.0,
                        help="mean grayscale ROI change that counts as visible")
    parser.add_argument("--output", type=Path,
                        default=Path("target/webcam-input-display-latency.json"))
    args = parser.parse_args()

    if args.samples <= 0:
        parser.error("--samples must be positive")

    camera = cv2.VideoCapture(args.camera_index, cv2.CAP_DSHOW)
    if not camera.isOpened():
        raise SystemExit(f"camera index {args.camera_index} could not be opened")
    ok, frame = camera.read()
    if not ok or frame is None:
        raise SystemExit("camera opened but returned no frame")
    x, y, width, height = args.roi
    frame_height, frame_width = frame.shape[:2]
    if x + width > frame_width or y + height > frame_height:
        raise SystemExit(f"ROI exceeds camera frame {frame_width}x{frame_height}")

    keydowns: list[float] = []
    stop = threading.Event()

    def on_press(key: keyboard.Key | keyboard.KeyCode) -> bool | None:
        if key == keyboard.Key.esc:
            stop.set()
            return False
        keydowns.append(time.perf_counter())
        return None

    listener = keyboard.Listener(on_press=on_press)
    listener.start()
    print("Calibrating camera ROI for two seconds; keep the monitor unchanged.")
    baseline_frames: list[float] = []
    calibration_deadline = time.perf_counter() + 2.0
    while time.perf_counter() < calibration_deadline:
        ok, frame = camera.read()
        if ok:
            crop = frame[y:y + height, x:x + width]
            baseline_frames.append(float(cv2.cvtColor(crop, cv2.COLOR_BGR2GRAY).mean()))
    baseline = statistics.median(baseline_frames[-30:])
    keydowns.clear()
    print("Ready. Press the chosen key only when the desktop visibly changes.")
    print("Press Esc to stop early.")

    samples: list[dict[str, float]] = []
    pending: float | None = None
    cooldown_until = 0.0
    while not stop.is_set() and len(samples) < args.samples:
        now = time.perf_counter()
        if pending is None and keydowns and keydowns[0] >= cooldown_until:
            pending = keydowns.pop(0)
        elif keydowns and keydowns[0] < cooldown_until:
            keydowns.pop(0)
        ok, frame = camera.read()
        if not ok:
            continue
        crop = frame[y:y + height, x:x + width]
        level = float(cv2.cvtColor(crop, cv2.COLOR_BGR2GRAY).mean())
        if pending is not None and abs(level - baseline) >= args.threshold:
            visible = now
            latency_ms = (visible - pending) * 1000.0
            samples.append({"latency_ms": round(latency_ms, 3),
                            "input_timestamp": pending,
                            "visible_timestamp": visible,
                            "frame_level": round(level, 3)})
            print(f"sample {len(samples)}/{args.samples}: {latency_ms:.3f} ms")
            pending = None
            cooldown_until = now + 0.75
            time.sleep(0.05)
            baseline = level
        elif pending is None and now >= cooldown_until:
            baseline = (baseline * 0.95) + (level * 0.05)

    stop.set()
    listener.stop()
    camera.release()
    if not samples:
        print("No samples captured; no latency claim was produced.")
        return 2
    values = [sample["latency_ms"] for sample in samples]
    ordered = sorted(values)
    p95 = ordered[min(len(ordered) - 1, max(0, (len(ordered) * 95 + 99) // 100 - 1))]
    result = {
        "status": "WEBCAM_ESTIMATE",
        "camera_index": args.camera_index,
        "roi": [x, y, width, height],
        "threshold": args.threshold,
        "sample_count": len(values),
        "latency_ms": values,
        "summary_ms": {
            "min": round(min(values), 3),
            "median": round(statistics.median(values), 3),
            "p95": round(p95, 3),
            "max": round(max(values), 3),
        },
        "limitations": [
            "host key-down is not the instant of physical switch closure",
            "camera exposure and frame period add measurement uncertainty",
            "this is not photodiode or oscilloscope certification",
        ],
        "samples": samples,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result["summary_ms"], indent=2))
    print(f"evidence: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
