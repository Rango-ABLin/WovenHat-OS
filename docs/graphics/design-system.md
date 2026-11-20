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

`ICON_GRAMMAR` fixes one geometry for all icon artwork: a 24 px grid, a 20 px
keyline, a 2 px stroke, a 6 px corner radius (grid/4, chosen so the shell tile's
established `size / 4` rounding is reproduced exactly) and a 32 px minimum
touch target. It also owns the tile's internal proportions (3 px inner inset,
and divisors of 5/10/4/3/2 for inner height, inner radius and glyph origin/span).
All invariants are asserted in a `const` block, so a malformed grammar cannot
compile into a guest image.

`IconGrammar::metrics(size)` derives one `IconMetrics` value from those rules.
A regression test locks the four sizes actually used by WovenShell (14, 32, 40
and 48 px) to their pre-convergence numbers, so this is a *single-source*
refactor, not a visual redesign.

`SemanticPalette::icon_ink` maps `icons::IconState` to one ink per state:
Normal → `text`, Active → `accent`, Disabled → `disabled`.

### Convergence completed 2026-11-20

The two producers now share one contract:

| Concern | Shared source | Previous duplication |
|---|---|---|
| Identity | `icons::IconId` (13 variants, pack v2) | shell-local 13-variant `AppIcon` enum |
| Name mapping | `IconId::from_app_name` | shell-local `app_icon()` match |
| Tile colors | `icons::IconAppearance::for_id` | shell-local `icon_colors()` table |
| Geometry | `design_system::ICON_GRAMMAR::metrics` | shell-local ad-hoc division |
| State ink | `SemanticPalette::icon_ink` | `IconTheme::ink` hard-coded values |

The shell's `AppIcon` is now a type alias of `IconId`, its `app_icon()` delegates
to the shared mapping, and `draw_app_icon()` reads the shared appearance and
geometry. `libwoven::icons` gained masks for the five identities that were
previously shell-only (Editor, Update, Ai, Monitor, Installer), so the 8×8 pack
covers the whole catalog. `IconTheme::ink` now delegates to the palette contract.
`main.rs` and every call site are unchanged, and the accepted tile pixels are
locked by tests.

The code-native, no-untrusted-SVG boundary is retained: there is no parser for
external icon files at this layer.

## 6. Motion

`Motion::{Instant, Fast, Normal, Slow}` delegates durations to
`ui::DesignTokens` (0/120/220/320 ms) and collapses to zero when reduced motion
is requested. Motion is ordered and asserted in host tests.

## 7. Weave wallpaper family

`design_system::wallpaper` is a pure, bounded 24-bit BMP renderer. It derives
each design from the same `WEAVE` identity as the live desktop, writes only into
caller-owned storage, uses integer math, and rejects zero/oversized geometry or
a short output buffer. Its worst case is bounded to 8192×8192 and 16 megapixels,
matching the image decoder's existing asset limits.

Three CC0-1.0 variants were generated at 320×200 (192,054 bytes each, well
inside the 256 KiB VFS node capacity, so no new kernel capability is needed):

| Variant | Palette | Weave | Intent |
|---|---|---|---|
| Dark | navy `(11,29,58)` → teal `(16,78,82)` | ±6 | Mirrors the live desktop; faint fabric over a clear gradient |
| Light | `(214,230,242)` → `(235,244,246)` | ±6 | Airy light-theme companion |
| High contrast | black → `(10,10,10)` | ±28 | Bold diagonal bands on black, behind high-contrast text |

The weave pitch scales with asset width (14 px at the established 160 px sample
width, 28 px at 320 px), the sample-scale analogue of the live 220 px pitch.

`libwoven/examples/generate_wallpapers.rs` is the **only** producer of the
committed BMPs and their manifests. It calls the same renderer the host tests
call, so the bytes cannot drift. Each manifest records id, CC0-1.0 license,
provenance, dimensions, `BMP24` format and SHA-256. A host test includes all
three assets, re-renders each from its spec and compares byte-for-byte, then
checks each manifest digest and license — so a hand-edited asset or stale digest
fails the test suite.

Visual QA was performed by converting the BMPs to PNG previews and inspecting
them; the dark variant's initial ±10 tint was reduced to ±6 after the stripes
were found to overwhelm the navy→teal gradient.

These assets are staged data only. Nothing in the kernel VFS bootstrap, the
shell, or the wallpaper decoder was changed, so no QEMU claim is made.

## 8. Sign-in pilot (in progress — visual acceptance blocked)

The first integration pilot applies the design system to the locked sign-in
surface only (`render_lock` in `apps/woven-shell`):

- Card fill and every text role come from `SemanticPalette::resolve`;
  avatar ink uses `on_accent` (8.12:1 on teal, versus white's ~2:1).
- Headline, account, prompt and clock sizes come from `TypeRole`, with
  `line_height_px()` supplying account-row leading.
- Account and prompt labels moved from 12 px to 14 px (Subtitle) after the
  baseline capture showed them too small at 1280×800.
- The chosen account row uses accent fill with `on_accent` ink.

The qualification harness now captures `sign-in.ppm` before PIN injection as a
reusable visual baseline. Early captures exposed a separate defect: `S` and `g`
vanished from the 20 px headline because `Canvas::draw_text_proportional` did
not treat an inkless cached real glyph as a miss, unlike `draw_text_ttf`. That
fallback has now been added and baseline-aligned to the real face's ascent
(commits `487eb003`, `dcf21259`). All characters render on one line; complex
outlines that still overflow the accepted 64-point `Path` budget use the bounded
bitmap substitute, which is the project's accepted design.

The verified capture was approved as the gate reference and is enforced by
`docs/graphics/reference-screenshots/baseline-manifest.json`. Evidence:

- Fresh 1-CPU `--display virtio-vga` run: `sign-in.ppm` vs approved reference —
  **0 / 1,024,000 changed pixels at 0.1% tolerance (PASS)**. The only gate failure
  is the pre-existing `overview` framebuffer flake.
- Negative control: the pre-fix capture differs by **25.54%** (261,508 pixels),
  which the comparator rejects — so the baseline is a real regression gate, not
  a decorative report.

Other scenes are explicitly deferred in the manifest: the signed-in desktop has
a live clock, the overview capture is timing-flaky, and later frames are
input-dependent. The baseline set is therefore one deterministic, approved
scene plus a mechanism ready for more.

## Validation evidence (2026-11-20)

Foundation increment:

- `cargo test -p libwoven --lib design_system` — 7/7 new tests passed.
- `cargo clippy -p libwoven --all-targets -- -D warnings` — clean at that time.
- `cargo clippy -p libwoven --target x86_64-unknown-none -- -D warnings` —
  clean (confirms `no_std`/freestanding compatibility).

Icon convergence increment:

- `cargo test -p libwoven --lib` — 253 passed, 0 failed, 1 pre-existing ignored
  (6 new tests across `icons` and `design_system`).
- `cargo clippy -p libwoven --lib -- -D warnings` — clean.
- `cargo clippy -p libwoven --target x86_64-unknown-none -- -D warnings` — clean.
- `cargo build --target x86_64-unknown-none` in `apps/woven-shell` — clean.
- `cargo clippy --target x86_64-unknown-none -- -D warnings` in
  `apps/woven-shell` — clean.

`cargo clippy -p libwoven --all-targets -- -D warnings` was transiently blocked
by three `unnecessary_cast` errors in `libwoven/tests/ttf_visual.rs` from
concurrent font work; that owner resolved them and the command is clean again.

Weave wallpaper increment:

- `cargo run -p libwoven --example generate_wallpapers` — generated all three
  BMPs and manifests from the spec.
- `cargo test -p libwoven --lib` — 256 passed, 0 failed, 1 pre-existing ignored
  (3 new wallpaper tests, including byte-for-byte asset and digest checks).
- `cargo clippy -p libwoven --all-targets -- -D warnings` — clean (includes the
  host generator example).
- `cargo clippy -p libwoven --target x86_64-unknown-none -- -D warnings` —
  clean (the renderer is `no_std`).

No QEMU run was performed for the design system, icon convergence or wallpaper
assets: no call site changed behavior and the accepted tile geometry is pinned
by exact-value tests. The concurrent owner recorded independent 1/2/4-CPU
desktop qualification results separately. Desktop integration of new visual
values still requires the normal qualification chain.

## Next prerequisites

1. Resolve the pre-existing `overview` framebuffer flake so the full desktop
   journey can pass 1/2/4 CPUs with the sign-in baseline enforced.
2. Approve further deterministic scenes when available (for example a fixed-time
   lock screen) and add them to `baseline-manifest.json`.
3. Migrate further type call sites to `TypeRole` leading with the bounded glyph
   cache, then re-run the full 1/2/4-CPU desktop qualification.
4. Only then propose the `ui::Theme::DARK.surface` correction as a separate,
   gated change with its own pixel-reference evidence.
5. When a wallpaper selector is wanted, stage these CC0 assets through the
   existing `vfs::create_read_only` path and add a gated shell pick-list; that
   is a separate, QEMU-qualified integration, not part of this asset increment.
6. Keep the all-targets Clippy check in the full gate; the transient
   `ttf_visual.rs` errors from concurrent font work are resolved.
