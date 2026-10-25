# Graphics roadmap Stage 05 typography increment — 2026-10-25

This increment adds the missing explicit font-provider boundary without
changing the existing renderer ABI. `libwoven::graphics::FontFallback` keeps a
caller-owned, priority-ordered chain of up to eight validated TrueType faces
and a fixed 128-entry round-robin glyph decision cache. Cache entries retain
only a Unicode scalar, validated glyph ID, face index, and advance metric; no
font bytes or raw pointers are retained by the cache. A scalar is selected
only when the face supplies both a cmap glyph and an advance metric.

The existing bounded TrueType/OpenType reader, cmap 4/12 lookup, kerning,
variation metadata, glyph outlines, hint subset, GSUB/GPOS subset, bidi subset,
word wrapping, and bitmap fallback remain intact. The new test exercises
priority selection, cache reuse, cache invalidation, and fail-closed missing
glyph behavior using the existing synthetic TrueType fixture.

The rendering fixture also rasterizes the bounded outline at 4x scale into an
80x80 surface and asserts deterministic non-empty coverage within the surface
bounds. The cmap12 fixture verifies a non-BMP scalar maps to a glyph and keeps
its advance metric; GSUB/GPOS, RTL, wrapping, fallback, and outline-render
tests remain in the same suite.

`assets/fonts/NotoSans-Regular.ttf` is bundled under the SIL Open Font
License with `assets/fonts/OFL.txt`. The real font validates through the
TrueType boundary and confirms Latin, Greek, and Cyrillic cmap/advance
coverage. The kernel now includes the same fixture, validates its bounded
sfnt/cmap/hmtx/glyf path, and rasterizes `WΩЖ` into a bounded buffer.

`libwoven::graphics::TextInputState` is the bounded input contract for
controls: it stores at most 512 UTF-8 bytes, keeps caret and selection offsets
on scalar boundaries, and supports insertion, deletion, backspace, and
selection-aware left/right movement without allocation.

Validation completed:

- `cargo test -p libwoven --quiet`: 149 passed, including the high-DPI raster
  and non-BMP cmap fixtures.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- `cargo fmt --package libwoven -- --check`: passed.
- `git diff --check`: passed.
- Existing guest font coverage gate `17.1` passed at 1 CPU, 2 CPU, and 4 CPU:
  - `audit-artifacts/stage17.1-1cpu-1792923579634997900`
  - `audit-artifacts/stage17.1-2cpu-1792923620066157000`
  - `audit-artifacts/stage17.1-4cpu-1792923639323565800`
- Kernel bundled-font load/raster gate `5-font` passed at 1 CPU, 2 CPU, and 4
  CPU:
  - `audit-artifacts/stage5-font-1cpu-1792924442556258600`
  - `audit-artifacts/stage5-font-2cpu-1792924470141690500`
  - `audit-artifacts/stage5-font-4cpu-1792924489585514900`

Stage 05 is accepted for the bundled static-font kernel/QEMU scope. The kernel
path is intentionally bounded to simple `glyf` outlines and fails closed for
composites or malformed data; full UAX #9 bidi, script shaping, IME, and
userspace font selection remain separate higher-level capabilities and are not
claimed by this gate.
