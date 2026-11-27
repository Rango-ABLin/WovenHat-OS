"""Guard Stage 20 framebuffer oracles against stale wallpaper geometry.

The QEMU Stage 20.1+ tests remain the authoritative pixel-level acceptance.
This host check catches a source-contract mismatch before the long QEMU run.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SHELL = ROOT / "apps" / "woven-shell" / "src" / "main.rs"
TOKENS = ROOT / "apps" / "woven-shell" / "src" / "tokens.rs"
KERNEL = ROOT / "kernel" / "src" / "main.rs"


class WallpaperOracleContractTests(unittest.TestCase):
    def test_shell_uses_diagonal_bands_without_grid_seams(self):
        source = SHELL.read_text(encoding="utf-8")
        tokens = TOKENS.read_text(encoding="utf-8")
        start = source.index("fn wallpaper(x: i32, y: i32, height: i32) -> u32 {")
        end = source.index("\n}\n", start) + 2
        wallpaper = source[start:end]
        # The live renderer consumes the token module; the accepted values are
        # checked there rather than assuming a duplicate declaration in main.
        self.assertIn("mod tokens;", source)
        self.assertIn("use tokens::*;", source)
        self.assertIn("pub const WEAVE_PITCH: i32 = 220;", tokens)
        self.assertIn("pub const WEAVE_TINT_OPACITY: u8 = 4;", tokens)
        self.assertIn("((x + y) / WEAVE_PITCH) % 2 == 0", wallpaper)
        self.assertIn("blend(tint, base, WEAVE_TINT_OPACITY)", wallpaper)
        self.assertNotIn("x % WEAVE_PITCH", wallpaper)

    def test_stage20_oracles_are_not_stale_checkerboards(self):
        source = KERNEL.read_text(encoding="utf-8")
        self.assertNotIn("x / 96 + y / 96", source)
        self.assertNotIn("x % 96 == 0 || y % 96 == 0", source)
        self.assertNotIn("WEAVE_OPACITY", source)
        # Both independent framebuffer oracles declare and apply the diagonal
        # weave locally; require both copies so one cannot silently regress.
        self.assertGreaterEqual(source.count("const WEAVE_PITCH: i32 = 220;"), 2)
        self.assertGreaterEqual(source.count("const WEAVE_TINT_OPACITY: u8 = 6;"), 2)
        self.assertGreaterEqual(source.count("((x + y) / WEAVE_PITCH) % 2 == 0"), 2)
        self.assertGreaterEqual(source.count("blend_pixel(tint, base, WEAVE_TINT_OPACITY)"), 2)

    def test_diagonal_pattern_changes_at_diagonal_boundary(self):
        # An independent boundary check: the old square-grid parity disagrees.
        x, y = 219, 1
        diagonal_band = ((x + y) // 220) % 2 == 0
        old_square_band = (x // 96 + y // 96) % 2 == 0
        self.assertFalse(diagonal_band)
        self.assertTrue(old_square_band)


if __name__ == "__main__":
    unittest.main()
