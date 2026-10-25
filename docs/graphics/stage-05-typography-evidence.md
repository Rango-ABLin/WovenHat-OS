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

`assets/fonts/NotoSans[wdth,wght].ttf` is bundled under the SIL Open Font
License with `assets/fonts/OFL.txt`. The real font validates through the
TrueType boundary and confirms Latin, Greek, and Cyrillic cmap/advance
coverage. Its variable-font outlines are not yet accepted by the bounded
outline decoder for raster output, so this does not claim bundled-font
rasterization.

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

Stage 05 remains partial. The repository still lacks a licensed production
font fixture packaged into the guest, a guest high-DPI multilingual rendering
gate, full UAX #9 bidi/script coverage, and guest wiring for the new
IME/selection/caret contract. The current guest gate verifies printable ASCII
coverage only. These results do not claim Stage 05 acceptance.
