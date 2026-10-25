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

Validation completed:

- `cargo test -p libwoven --quiet`: 147 passed.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- `cargo fmt --package libwoven -- --check`: passed.
- `git diff --check`: passed.

Stage 05 remains partial. The repository still lacks a licensed production
font fixture packaged into the guest, a guest high-DPI multilingual rendering
gate, full UAX #9 bidi/script coverage, and complete IME/selection/caret
integration. This commit does not claim those capabilities or stage
acceptance.
