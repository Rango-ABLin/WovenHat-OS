"""Compare a captured framebuffer against a saved reference image.

Closes the Phase 2/9 gap noted in docs/graphics/modernization-tracker.md:
both phases' acceptance criteria ask for automated pixel/tolerance
comparison against reference images, not just manual inspection or the
existing "did the frame change at all" journey checks in
test-desktop-qualification.py.

Usage:
    python scripts/compare-against-reference.py \
        --candidate path/to/live-capture.ppm \
        --reference path/to/reference.png \
        --tolerance-percent 0.5

Exits 0 and prints a PASS summary when the fraction of differing pixels is
at or below --tolerance-percent; exits 1 and prints a FAIL summary
otherwise (dimension mismatch is always a FAIL, regardless of tolerance).
Both PPM (P6) and any Pillow-readable format (PNG, etc.) are accepted for
either side; reference images are expected to live under
docs/graphics/reference-screenshots/ with a short description of what they
capture and when they were captured/approved, committed alongside the code
that produces them.
"""

import argparse
import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _load_qualification_module():
    """Reuse the PPM parser from test-desktop-qualification.py rather than
    duplicating it -- one parser, one set of edge cases to get right."""
    spec = importlib.util.spec_from_file_location(
        "desktop_qualification", ROOT / "scripts" / "test-desktop-qualification.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _read_pixels(path: Path, qualification) -> tuple[int, int, bytes]:
    if path.suffix.lower() == ".ppm":
        return qualification.parse_ppm(path)
    from PIL import Image

    image = Image.open(path).convert("RGB")
    return image.width, image.height, image.tobytes()


def compare(candidate: Path, reference: Path, tolerance_percent: float) -> dict:
    qualification = _load_qualification_module()
    candidate_w, candidate_h, candidate_pixels = _read_pixels(candidate, qualification)
    reference_w, reference_h, reference_pixels = _read_pixels(reference, qualification)

    if (candidate_w, candidate_h) != (reference_w, reference_h):
        return {
            "pass": False,
            "reason": (
                f"dimension mismatch: candidate {candidate_w}x{candidate_h} "
                f"vs reference {reference_w}x{reference_h}"
            ),
            "candidate": str(candidate),
            "reference": str(reference),
        }

    total_pixels = candidate_w * candidate_h
    changed = qualification.changed_pixels(candidate_pixels, reference_pixels)
    changed_fraction_percent = (changed / total_pixels) * 100 if total_pixels else 0.0

    max_channel_delta = 0
    for offset in range(0, len(candidate_pixels), 3):
        for channel in range(3):
            delta = abs(
                candidate_pixels[offset + channel] - reference_pixels[offset + channel]
            )
            if delta > max_channel_delta:
                max_channel_delta = delta

    passed = changed_fraction_percent <= tolerance_percent
    return {
        "pass": passed,
        "candidate": str(candidate),
        "reference": str(reference),
        "width": candidate_w,
        "height": candidate_h,
        "total_pixels": total_pixels,
        "changed_pixels": changed,
        "changed_fraction_percent": round(changed_fraction_percent, 4),
        "tolerance_percent": tolerance_percent,
        "max_channel_delta": max_channel_delta,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--candidate", type=Path, required=True, help="Live-captured PPM or image to check")
    parser.add_argument("--reference", type=Path, required=True, help="Approved reference image to compare against")
    parser.add_argument(
        "--tolerance-percent",
        type=float,
        default=0.1,
        help="Maximum fraction of differing pixels, as a percentage, to still pass (default: 0.1)",
    )
    args = parser.parse_args(argv)

    if not args.candidate.is_file():
        parser.error(f"candidate not found: {args.candidate}")
    if not args.reference.is_file():
        parser.error(f"reference not found: {args.reference}")
    if args.tolerance_percent < 0:
        parser.error("--tolerance-percent must not be negative")

    result = compare(args.candidate, args.reference, args.tolerance_percent)
    status = "PASS" if result["pass"] else "FAIL"
    print(f"[{status}] {result}")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
