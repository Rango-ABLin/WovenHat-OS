# WovenHat Design System (WHDS) v1 — brand foundation

Date: 2026-11-20
Owner: OpenCode (graphics design mandate)
Status: **IMPLEMENTED, ISOLATED, HOST-VALIDATED** — no desktop call site has
migrated yet. No kernel ABI, compositor, input-routing or accepted gate is
changed by this increment.

## Authority and scope

The user authorized a complete graphics/font/icon design effort for WovenHat OS
and selected three constraints for the opening increment: **refine the existing
weave identity**, build the **design-system foundation first**, and keep the
first pass **isolated and validated**. This document is the specification for
that foundation. The implementation lives in `libwoven/src/design_system.rs`.

This is deliberately additive. `libwoven::ui::Theme` and
`libwoven::ui::DesignTokens` remain the accepted primitives used by first-party
apps. The new module reads their constants directly for shared roles, so the
two cannot drift, and adds only roles they do not express: a background role,
a warning role, a real type scale with leading, a single icon grammar, and
measured WCAG contrast checks.

Out of scope for v1 (sequenced, not abandoned): changing `ui::Theme`,
migrating shell or app call sites, redesigning individual icon artwork,
generating wallpaper bitmaps, adding a font or image asset, touching the
kernel/VFS, and any QEMU acceptance claim.

## 1. Brand identity — the weave

The existing desktop already has one genuine visual signature: a vertical navy
to teal gradient over a faint diagonal fabric weave. WHDS promotes it from an
implicit shell constant to a documented identity shared with asset generators.

| Role | Value | Existing source |
|---|---|---|
| Canvas top | `rgb(11, 29, 58)` | shell `WALLPAPER_TOP` |
| Canvas bottom | `rgb(16, 78, 82)` | shell `WALLPAPER_BOTTOM` |
| Weave tint / shade | white / black | shell `WEAVE`, `WEAVE_SHADE` |
| Weave opacity | `6 / 255` | shell `WEAVE_TINT_OPACITY` |
| Weave pitch | `220 px` | shell `WEAVE_PITCH` |
| Brand accent | `rgb(76, 190, 188)` | `Theme::DARK.accent`, shell `ACCENT` |
| Accent deep | `rgb(28, 115, 145)` | shell `START_FILL` |

`Weave::phase`, `Weave::is_highlight` and `Weave::sample` are pure `const fn`s.
The motif is periodic along `x + y`, uses wrapping addition so no coordinate
can overflow, and lets a wallpaper generator and the live renderer agree on the
same pattern without sharing a framebuffer or a runtime dependency.

The wide 220-pixel pitch and lack of a seam line are intentional: axis-aligned
blocks of this scale previously read as a checkerboard rather than fabric.

## 2. Semantic palette

`SemanticPalette` has three const instances — `DARK`, `LIGHT`,
`HIGH_CONTRAST` — and `resolve(high_contrast, light)` matches the existing
accessibility switches (high contrast wins). Shared roles are read from
`ui::Theme::*`; brand-specific additions are `background`, `surface` and
`warning`.

The dark `surface` is intentionally **deeper than `Theme::DARK.surface`** for
measured legibility (see section 3). `surface_raised` is one elevation step
above it. High contrast is black/white with saturated signal colours and no
decorative subtlety.

## 3. Accessibility — measured, not guessed

`relative_luminance` uses an exact 256-entry linear-sRGB table (IEC 61966-2-1,
scaled so 1.0 == 10000) and Rec. 709 coefficients in integer math.
`contrast_ratio` returns hundredths (`450` == 4.5:1). `meets_aa`,
`meets_aa_large` and `meets_non_text` encode the WCAG thresholds. No floating
point, no allocation, no `std`.

Measured contrast of every semantic pairing (values are the implementation's
integer results):

| Pairing | Dark | Light | High contrast | Requirement |
|---|---:|---:|---:|---|
| `text` / `surface` | 14.35 | 16.38 | 21.00 | AA body 4.5 |
| `text_muted` / `surface` | 6.48 | 5.32 | 21.00 | AA body 4.5 |
| `focus` / `surface` | 11.05 | 5.21 | 16.74 | non-text 3.0 |
| `success` / `surface` | 8.12 | 5.65 | 16.53 | AA body 4.5 |
| `warning` / `surface` | 8.89 | 5.68 | 13.62 | AA body 4.5 |
| `danger` / `surface` | 4.74 | 6.38 | 8.65 | AA body 4.5 |
| `on_accent` / `accent` | 8.12 | 4.77 | 19.55 | AA body 4.5 |

**Concrete finding.** `ui::Theme::DARK.surface` is `rgb(45, 65, 90)`. Against
it the shared `muted` ink measures **4.12:1** and `danger` **3.01:1**, both below
AA body-text 4.5:1. The design-system dark surface `rgb(20, 32, 50)` raises
those to 6.48:1 and 4.74:1. The accepted theme is *not* modified here; this is
recorded evidence for a future, separately gated theme migration. A regression
test asserts both the failure of the old pairing and the success of the new
one, so a future "simplification" cannot silently reintroduce the defect.

`border` is a one-pixel separator rather than a control boundary, so it is not
held to WCAG 1.4.11's 3:1; `focus` is, and is asserted.

## 4. Type scale

Em sizes are delegated to `ui::DesignTokens::DEFAULT` (caption 10, body 12,
subtitle 14, title 16, headline 20, display 28 px). `TypeRole` adds leading as
a percentage of the em (130/140/135/125/120/112 respectively) and clamps line
height to at least `em + 1`.

| Role | Em | Leading | Line height |
|---|---:|---:|---:|
| Caption | 10 | 130% | 13 |
| Body | 12 | 140% | 16 |
| Subtitle | 14 | 135% | 18 |
| Title | 16 | 125% | 20 |
| Headline | 20 | 120% | 24 |
| Display | 28 | 112% | 31 |

This closes a real layout defect in the bitmap path: it drew a 9 px line for a
12 px em, so multi-line text overlapped. The broad outline-font migration was
previously rejected after real QEMU testing because full outline rasterization
blew the 2-CPU Files-launch IPC bound; a correct migration must use the bounded
64-entry glyph cache and now has a defined leading contract to migrate onto.

## 5. Icon grammar

`ICON_GRAMMAR` fixes one geometry for all future icon artwork: a 24 px grid, a
20 px keyline, a 2 px stroke, a 5 px corner radius and a 32 px minimum touch
target. The four geometry invariants are asserted in a `const` block, so a
malformed grammar cannot compile into a guest image.

`SemanticPalette::icon_ink` maps `icons::IconState` to one ink per state:
Normal → `text`, Active → `accent`, Disabled → `disabled`. WovenHat currently
has two icon producers — the shared 8×8 code-native `libwoven::icons` pack and
the shell-local 12-app vector tile painter — with different colors and grids.
This grammar is the convergence target; neither producer is changed in v1.

The code-native, no-untrusted-SVG boundary is retained: there is no parser for
external icon files at this layer.

## 6. Motion

`Motion::{Instant, Fast, Normal, Slow}` delegates durations to
`ui::DesignTokens` (0/120/220/320 ms) and collapses to zero when reduced motion
is requested. Motion is ordered and asserted in host tests.

## Validation evidence (2026-11-20)

- `cargo test -p libwoven --lib design_system` — 7/7 new tests passed.
- `cargo test -p libwoven --lib` — 250 passed, 0 failed, 1 pre-existing ignored.
- `cargo clippy -p libwoven --all-targets -- -D warnings` — clean.
- `cargo clippy -p libwoven --target x86_64-unknown-none -- -D warnings` —
  clean (confirms `no_std`/freestanding compatibility).

No QEMU run was performed because no rendering behavior changed. Desktop
integration will require the normal 1/2/4-CPU qualification chain.

## Next prerequisites

1. Converge the 8×8 pack and shell tile painter onto `ICON_GRAMMAR` and one
   `icon_ink` source (isolated, host-testable).
2. Generate weave-family wallpaper variants as bounded BMPs for the existing
   VFS staging path (no new syscall or decoder).
3. Migrate type call sites to `TypeRole` leading with the bounded glyph cache,
   then re-run the full 1/2/4-CPU desktop qualification.
4. Only then propose the `ui::Theme::DARK.surface` correction as a separate,
   gated change with its own pixel-reference evidence.
