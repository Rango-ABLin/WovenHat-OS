# Prompt for Claude Code: WovenHat "Overview" desktop (GNOME-style workflow, original WovenHat design)

Paste everything below the line into Claude Code (or tell it to read this file).

---

## 0. Role and goal

You are Claude Code working in `C:\WovenHat-OS` (Rust, `no_std` userspace on the WovenHat kernel, x86_64, QEMU on Windows). Build a **new desktop layout for WovenShell** that follows the *workflow ideas* of the GNOME shell (top bar, Activities overview, dash, workspaces, notification/calendar popover, system menu, keyboard-first navigation) but is an **original WovenHat design**: WovenHat palette and weave motif, its own names, its own icons. Do **not** copy GNOME or Adwaita code, icons, wallpapers, logos or names, and do not call anything "GNOME". Suggested user-facing name: **WovenHat Overview**.

**This is an authorized full desktop overhaul.** The user has decided: the Overview layout *replaces* the bottom-taskbar layout as the one WovenHat desktop (new default). It lives inside `apps/woven-shell` (no second shell binary, no new OS service), and it is delivered in gated milestones so the desktop stays bootable at every commit. **Claude Code owns the WovenHat shell and takes over all desktop-shell work**; any earlier shell assignment is superseded. Reuse and migrate the existing engine (compositor protocol, windowing, notifications, lock/login, workspaces, tiling, theming hooks); replace the chrome, navigation model and visual language wholesale. The old taskbar layout is deleted only at M7, after the new one passes every gate; until then keep it behind `--layout=taskbar` as the rollback path.

## 1. Mandatory first steps (do these before editing anything)

1. Read `AGENTS.md`, `docs/master-development-roadmap.md`, `docs/stage-status.md`, `docs/architecture-and-codebase-guide.md`, `docs/graphics/modern-desktop-implementation-2026-10-29.md`, `docs/graphics/desktop-acceptance-2026-10-27.md`, and `docs/agent-coordination.md`. Follow the rules in `AGENTS.md` exactly (stage order, safe Rust, stage feature gates, no weakened tests, evidence preserved, separate commits per accepted stage).
2. **Ownership (decided by the user: Claude Code takes over the WovenHat shell).** Ignore `CODEX-START-HERE.md`, `docs/CODEX-ASSIGNMENT.txt`, `docs/CODEX-Windows11-Assignment.txt` and `docs/Windows11-Desktop-Specification.md`: they are superseded and must not be implemented. Do not create `wovenhat-gnome-shell` or any other new shell crate; the work is the existing WovenHat shell, `apps/woven-shell`. Extend the seeded "Claude Code - WovenHat Overview desktop overhaul" entry in `docs/agent-coordination.md`, claiming `apps/woven-shell/**`, the desktop journeys in `scripts/test-desktop-qualification.py`, and the new docs. If another agent has uncommitted edits in those paths, stop and tell the user; otherwise proceed.
3. Treat `docs/GNOME-Desktop-Implementation-Brief.md` as a *wishlist only*. Its architecture (new `wovenhat-gnome-shell` and `wovenhat-settings-daemon` crates, 8-day timeline, 50 MB / 60 FPS targets) does not match the code. Replace those targets with measured budgets (section 6).

## 2. What already exists (verified in the repo, 2026-11-03)

- `apps/woven-shell/` is its own cargo workspace (excluded from the root workspace), target `x86_64-unknown-none`, crate `wovenhat-woven-shell`, depends only on `libwoven`. `src/main.rs` is ~4,650 lines (single file; `src/input.rs` is tiny). Document header in `main.rs` describes the protocol and features.
- Existing features: procedural wallpaper + weave overlay; bottom translucent taskbar (24 px logical) with WovenStart button, window entries, 4-workspace switcher, tray, clock; launcher panel (F1) reading `/share/applications/*` (`Name=`, `Exec=` with `%p`); F2 tiling; F3/F4 workspaces; edge snap by drag; Notification Center (last 8, unread badge) and toasts (4 s, DND); Quick Settings (display scale 1x/2x, DND, tiling, lock); lock screen and login (PBKDF2 PIN, lockout); multi-monitor (primary hosts chrome); spring-animated window fade-in; shadows; display scale 2 at >=1280x800; high-contrast theme helpers (`theme_*`).
- Shell IPC (4-byte tags): `WOPN`, `TRAY`, `NOTE`, `WPMV`, `WPBT`, `WCAP`/`WCAN`, `WKEY`, `WSIZ`/`WRSZ`, `WCLS`. Preserve every tag and its semantics; additions must be versioned/opt-in, bounded and sender-authenticated.
- Hard limits in code today: `MAX_WINDOWS = 4`, `WORKSPACES = 4`, `MAX_APPS = 64`, `MAX_NOTES = 8`, client surfaces capped at 320x240, shell composites in 320x40 tiles into one staging surface with static texture buffers (`UnsafeCell` statics). Keep everything statically bounded; no heap growth per frame.
- Rendering: `libwoven::graphics::Renderer` (anti-aliased rounded rects, alpha, gradients, 3D opt-in), `libwoven::layout`, `libwoven::icons`, `libwoven::i18n`, `libwoven::accessibility`, `libwoven::ui` (Button, etc.), `Spring` animation. Text is the **5x7 bitmap font** (`kernel/src/font.rs`, advance 6, line 9). `assets/fonts/NotoSans-Regular.ttf` exists and graphics Stage 05 (typography) is an accepted boundary: investigate before choosing; do not invent a font stack.
- Status reality: the "Modern desktop implementation" is **IMPLEMENTING / NOT ACCEPTED**; its ordered prerequisites (input/accessibility reliability, shared visual identity, typography/icons, persistent personalization, window/desktop navigation, ...) are not complete. The WCAN saturated-mailbox retry is unqualified. The grep found **no Super/Meta key handling** in the shell. Verify what keys the HID/keyboard path can deliver (`kernel/src/keyboard.rs`, `hid.rs`, `libwoven::Key`) before promising a Super key; the fallback is F1 and a hot corner.
- Persistent theme/accent/wallpaper writes need an authorized per-user mechanism; there is **no broad `/etc` write capability**. Do not add one to satisfy this feature.

## 3. Design to deliver (layout `overview`)

Implement inside `apps/woven-shell` (split `main.rs` into modules as you go, e.g. `tokens.rs`, `top_bar.rs`, `overview.rs`, `dash.rs`, `system_menu.rs`, `calendar.rs`, `switcher.rs`, `search.rs`; keep each step behavior-preserving and committed separately). Gate the new layout so the legacy layout is unchanged.

1. **Design tokens**: one module for colors, radii, spacing, elevations, motion durations, built from the existing WovenHat palette (`ACCENT`, wallpaper gradient, weave motif); light/dark + accent + high-contrast variants; reduced-motion switch.
2. **Top bar** (logical height scales with display scale): left "Activities" button/hot corner; center clock + date that opens the calendar/notification popover (reuse Notification Center state, unread badge, DND); right status cluster (tray icons, network/volume/power only where the system genuinely has the service, otherwise omit - the existing rule: "settings the system cannot act on yet are not offered") and a system menu (reuse Quick Settings actions: scale, DND, tiling, lock, plus sign-out if the identity service supports it).
3. **Activities overview**: opened by Activities button, hot corner, F1 (and Super if verified). Contains: search field (filters `/share/applications`, bounded `MAX_QUERY`), window thumbnails of the current workspace (scaled region blits of client shared surfaces; if the graphics ABI has no scaled blit, document it and render title-card placeholders rather than adding kernel capability without review), a workspace strip for the 4 workspaces with drag-to-move-window, and an app grid (paged, bounded). Enter launches the top result; Esc closes; Tab/arrow keyboard navigation with a visible focus ring.
4. **Dash**: favorites + running apps (bounded list, persisted only through an authorized mechanism or deferred), launch/focus on click.
5. **Window management**: Alt+Tab switcher with focus restoration, Super/Alt+arrows snap halves (reuse existing snap code), keep tiling (F2), workspace keys (F3/F4 stay). Consider raising `MAX_WINDOWS` only with a review of every static buffer, hit-test loop and shadow/composition cost; otherwise keep it and say so.
6. **Notifications**: toast under the top bar center, history in the calendar popover, DND honored, lock-screen rule preserved (remembered, never shown).
7. **Motion**: spring/ease transitions for overview open/close and workspace slide, <=300 ms, disabled under reduced motion; frame clock only while animating (existing 20 ms/250 ms behavior).
8. **Accessibility and localization**: every control reachable by keyboard; use `libwoven::accessibility` and `i18n` catalogs (including Chichewa) for all new strings; high-contrast variant of every new surface.
9. **Settings integration**: only through existing services and the Settings app; no broadened capabilities. Anything dependent on the unaccepted personalization stage is stubbed behind a clearly documented TODO, not faked.

## 4. Engineering rules (from AGENTS.md, restated)

- Prefer safe Rust. New `unsafe` only where the existing static-texture pattern requires it; document and minimize it. No new syscall, kernel object or capability unless unavoidable; if needed, stop and ask the user with a written threat review (userspace pointer validation, owner teardown, cancellation, generation reuse, bounded capacity, lock order, preemption, cross-CPU races).
- Event-driven blocking only. Never sleep or switch tasks under an IRQ lock.
- Failures are defects to diagnose. Do not weaken assertions, disable tests, suppress warnings or raise timeouts.
- Each stage self-test call site needs its own `#[cfg(feature = "...")]` guard; lint every affected stage feature (`cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features <stage> -- -D warnings`) and `cargo clippy --all-targets -- -D warnings`.
- Run builds and QEMU with the project-local Cargo dirs set by `RUN-STAGE10.7.ps1`. Run one acceptance chain at a time (fixed ports/paths). Do not kill unrelated QEMU processes; do not delete historical evidence in `audit-artifacts/`.
- Shell lints: `cargo clippy --target x86_64-unknown-none -- -D warnings` inside `apps\woven-shell`, plus host unit tests for pure logic (layout math, hit testing, search filtering, state machines) via the existing `#[cfg(test)]` pattern.

## 5. Milestones (each is a separate commit; do not start the next until the gate passes)

| # | Deliverable | Gate |
|---|---|---|
| M0 | Coordination decision recorded; plan and claimed files in `docs/agent-coordination.md` | User confirmed layout strategy |
| M1 | Behavior-preserving split of `main.rs` into modules + tokens | Shell lint + host tests + existing desktop qualification unchanged |
| M2 | `--layout=overview` top bar + clock/calendar popover + system menu | Screenshot + keyboard journey on 1/2/4 CPUs |
| M3 | Activities overview: search, workspace strip, window cards, app grid | Journey: open, search, launch, close; focus restored |
| M4 | Dash, Alt+Tab, snap shortcuts, notifications placement | Journey with 3+ windows, workspace move |
| M5 | Motion, reduced motion, high contrast, i18n, accessibility pass | Keyboard-only journey, contrast screenshots |
| M6 | Measured performance/memory record and docs | Budgets in section 6 met or honestly reported |
| M7 | Make Overview the default; remove the legacy taskbar chrome and dead code; installer/recovery/hello-gui/app fixtures verified on the new desktop | Full `RUN-STAGE10.7.ps1` Windows gate plus desktop journeys on 1/2/4 CPUs; all prior-stage acceptance still passes |

Overhaul scope beyond the shell: update every first-party app under `apps/` (Files, Settings, Terminal, Editor, Calculator, Monitor, Store, Search, Security, Images, Installer, Update, Recovery, AI) to the new visual tokens, window chrome and keyboard conventions through `libwoven::ui`, without breaking their IPC or accepted behavior; add a new original wallpaper set and icon set under `assets/` with license notes (generate procedurally or author originals; no third-party GNOME/Adwaita art).

Extend `scripts/test-desktop-qualification.py` (real QEMU, framebuffer capture) with overview journeys rather than creating a parallel harness. Run at 1, 2 and 4 CPUs; preserve serial logs and screenshots under `audit-artifacts/`.

## 6. Budgets (measure, do not assume)

Establish baseline numbers first from the existing desktop: frame time of a full recompose, shell resident memory, time from boot to responsive desktop, search latency over 64 apps. Then require: overview open/close animation at the existing frame clock without dropped input; no regression of legacy layout timings; shell static memory increase reported in the docs. Report real numbers, including failures.

## 7. Documentation and reporting

Update `docs/stage-status.md`, `docs/architecture-and-codebase-guide.md`, a new `docs/graphics/overview-desktop-<date>.md` (design, protocol additions, limits, evidence paths), and `docs/agent-coordination.md`. State plainly which gates passed and which did not; source integration or a focused pass is not stage acceptance. Commit each accepted milestone separately with the required `Co-authored-by` trailer used in this repo's workflow. End every session with: what changed, what was verified (commands + results), what remains, and next prerequisites.

## 8. Out of scope (stop and ask if you think you need these)

Copying GNOME assets/code; a second shell binary or new top-level service crates; broad filesystem-write capabilities; browser/GPU/Wayland compatibility; hardware-specific claims (native hardware qualification is a separate gate).
