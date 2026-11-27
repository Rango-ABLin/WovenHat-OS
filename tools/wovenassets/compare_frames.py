#!/usr/bin/env python3
"""Host-only deterministic screenshot comparison. Exit: 0 pass, 1 mismatch, 2 invalid input."""
import argparse
import json
import math
import sys
from pathlib import Path
from PIL import Image, ImageChops, ImageStat


def compare(reference, actual, max_abs=0, mean_abs=0.0, out=None):
    if type(max_abs) is not int or not 0 <= max_abs <= 255:
        raise ValueError("max_abs must be an integer in [0, 255]")
    if type(mean_abs) not in (int, float) or not math.isfinite(mean_abs) or not 0 <= mean_abs <= 255:
        raise ValueError("mean_abs must be finite and in [0, 255]")
    # Remove any old success-looking diagnostic before attempting new comparison.
    if out is not None:
        Path(out).unlink(missing_ok=True)
    with Image.open(reference) as ri, Image.open(actual) as ai:
        if ri.size != ai.size:
            return {"pass": False, "reason": "dimension_mismatch",
                    "reference": list(ri.size), "actual": list(ai.size)}
        r = ri.convert("RGBA")
        a = ai.convert("RGBA")
        diff = ImageChops.difference(r, a)
        extrema = diff.getextrema()
        maximum = max(value[1] for value in extrema)
        mean = sum(ImageStat.Stat(diff).mean) / 4
        if out is not None:
            diff.save(out)
        return {"pass": maximum <= max_abs and mean <= mean_abs,
                "size": list(r.size),
                "max_absolute_channel_difference": maximum,
                "mean_absolute_channel_difference": round(mean, 6)}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("reference")
    p.add_argument("actual")
    p.add_argument("--max-abs", type=int, default=0)
    p.add_argument("--mean-abs", type=float, default=0.0)
    p.add_argument("--diff-png")
    a = p.parse_args()
    try:
        result = compare(a.reference, a.actual, a.max_abs, a.mean_abs, a.diff_png)
        print(json.dumps(result, indent=2))
        return 0 if result["pass"] else 1
    except (OSError, ValueError) as error:
        print(json.dumps({"error": str(error)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
