# Graphics roadmap Stage 06 iconography evidence

Date: 2026-10-25

## Implementation

`libwoven::icons` is the versioned shared icon contract. Pack version 1 covers
Files, Terminal, Store, Settings, Security, Images, Calculator, and Generic
icons, with stable names and MIME associations. The bounded 8x8 vector-like
masks render at integer DPI scales 1..16 and expose Normal, Active, and
Disabled states plus Light and Dark theme ink. WovenShell now consumes the
same pack for launcher and desktop tiles, avoiding shell-local glyph drift.

This increment intentionally does not add an untrusted SVG parser. External
SVG conversion and package asset import remain a later, capability-checked
pipeline; the current pack is code-native and cannot execute or allocate on
untrusted asset input.

## Evidence

- `cargo test -p libwoven --quiet`: 152 passed.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- Root kernel release build and warning-denying freestanding kernel Clippy:
  passed.
- All 17 Ring-3 app manifests build in their freestanding release profiles;
  the recovery app now has the same local `core+alloc` target configuration.
- WovenShell release build: passed.
- `python scripts/test-woven-shell-qemu.py --stage 20.3 --cpus 1`: passed,
  exit 33; evidence `target/woven-shell-launcher-1cpu`.
- The same launcher acceptance passed at 2 and 4 CPUs; evidence is under
  `target/woven-shell-launcher-2cpu` and `target/woven-shell-launcher-4cpu`.

## Gate and limits

Stage 06 is accepted for the shared code-native icon pack, shell integration,
theme/state variants, and supported QEMU 1/2/4-CPU coverage. Physical display
sharpness, GPU icon acceleration, third-party SVG import, and a complete
external MIME thumbnail service are not claimed by this stage.

## Addendum 2026-11-20 — additive v2 convergence (does not alter acceptance)

A later design-system increment promoted the pack to version 2 and converged
both icon producers onto one shared contract, without changing this stage's
accepted evidence or any rendered tile geometry. See
`docs/graphics/design-system.md` for the full specification.

- `IconId` gained the five shell-only identities (Editor, Update, Ai, Monitor,
  Installer) and an `IconId::from_app_name` name mapping; the 8×8 mask table
  covers all 13 identities.
- The shell tile painter's private appearance table was moved into
  `icons::IconAppearance`, its `AppIcon` became an alias of `IconId`, and its
  geometry now comes from `design_system::ICON_GRAMMAR::metrics`.
- `IconTheme::ink` now delegates state colors to `SemanticPalette::icon_ink`.
- Exact-value host tests pin the accepted tile geometry at the four shell sizes
  (14/32/40/48 px). `apps/woven-shell` builds and passes warning-denying target
  Clippy; libwoven host tests pass 253/253.

This is a single-source refactor, not a visual redesign, and makes no new QEMU
claim. The original Stage 06 acceptance results above remain unchanged.
