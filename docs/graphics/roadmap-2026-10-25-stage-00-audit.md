# Graphics engineering roadmap — Stage 00 live audit

Date: 2026-10-25  
Repository: `Rango-ABLin/WovenHat-OS`  
Branch: `master`  
Audit scope: the complete Stage 00–24 graphics roadmap supplied in
`WovenHat_OS_Complete_Graphics_Engineering_Roadmap_and_Codex_Mandate.pdf`.

This is a live repository audit, not an acceptance claim. The roadmap requires
each stage to pass its implementation, host, target, and runtime gates before
the next mandatory stage is accepted.

## Baseline evidence

- Current head: `8c894164 browser: add bounded borrowed html document parser`.
- Recent supporting commits: `026a93c9` media boundary documentation,
  `9e29d508` bounded image metadata parsing, `42da105d` web boundary
  documentation, `29d9cc6d` bounded WebView host boundary, and the shared UI
  control migration commits immediately preceding them.
- `cargo test -p libwoven --quiet`: 147 passed, 0 failed.
- `cargo test -p libwoven browser::tests`: 2 passed, 0 failed.
- Settings target warning-denying Clippy passed:
  `cargo clippy --manifest-path apps/woven-settings/Cargo.toml --target x86_64-unknown-none -- -D warnings`.
- Rust formatting and `git diff --check` passed for the affected graphics,
  media, web, and UI sources.
- The initial QEMU desktop qualification reached the 5 GiB guest memory floor
  but timed out waiting for `woven-shell: signed in: ada`. The harness was
  corrected to use explicit headless-QEMU key press/release events and to
  preserve whitespace-valued first pixels in binary PPM frames. The corrected
  gate passed serially at 1, 2, and 4 CPUs. Evidence is preserved under
  `audit-artifacts/desktop-qualification-1792921718604134100`,
  `audit-artifacts/desktop-qualification-1792921848678156500`, and
  `audit-artifacts/desktop-qualification-1792921970674483800`.
- A lower-memory app run was correctly rejected at the 4 GiB floor; evidence
  is preserved under `audit-artifacts/update-desktop-1cpu-1792917732442264000`.
- No physical GPU, hardware display, codec, TLS, JavaScript, or native
  hardware certification is claimed by this audit.

## Current reconciliation - 2026-10-27

The original audit below is historical. Dedicated Stage 01-09 evidence files
subsequently accepted their explicitly bounded software/QEMU slices; consult
those files rather than treating the original PARTIAL rows as live failures.
The software desktop closure at cea33d1a passes 87 required cases and the
strict GFX-07 nine-case checker; see desktop-acceptance-2026-10-27.md.
This does not accept full native hardware, GFX-08 or the entire mandate.

The current ordered work is (1) window/input/accessibility reliability,
(2) authorized persistent settings, (3) measured performance qualification,
(4) named native-device qualification. Step 1 is in progress, not accepted;
steps 2-4 must not advance until its gate passes. The new independent
Stage20.19 fixture covers capture, cancellation, occlusion and keyboard focus.

## Stage ledger

| Stage | Requirement | Live classification | Evidence / gap |
|---:|---|---|---|
| 00 | Audit, baseline, evidence | **ACCEPTED** | Live repository audit is recorded here; corrected real-QEMU desktop qualification passed serially at 1/2/4 CPUs with 5 GiB configured RAM and preserved evidence. |
| 01 | Safety and graphics architecture contracts | ACCEPTED for documented software/QEMU boundary | See dedicated stage-01 evidence/contract file; its stated limitations remain in force. |
| 02 | Display HAL and modesetting | ACCEPTED for documented software/QEMU boundary | See dedicated stage-02 evidence/contract file; its stated limitations remain in force. |
| 03 | Buffered presentation and damage | ACCEPTED for documented software/QEMU boundary | See dedicated stage-03 evidence/contract file; its stated limitations remain in force. |
| 04 | 2D raster/vector geometry | ACCEPTED for documented software/QEMU boundary | See dedicated stage-04 evidence/contract file; its stated limitations remain in force. |
| 05 | Fonts, Unicode, shaping, typography | ACCEPTED for documented software/QEMU boundary | See dedicated stage-05 evidence/contract file; its stated limitations remain in force. |
| 06 | Iconography and asset system | ACCEPTED for documented software/QEMU boundary | See dedicated stage-06 evidence/contract file; its stated limitations remain in force. |
| 07 | Images, wallpapers, asset packaging | ACCEPTED for documented software/QEMU boundary | See dedicated stage-07 evidence/contract file; its stated limitations remain in force. |
| 08 | Secure userspace graphics surfaces and IPC | ACCEPTED for documented software/QEMU boundary | See dedicated stage-08 evidence/contract file; its stated limitations remain in force. |
| 09 | Compositor and scheduler | ACCEPTED for documented software/QEMU boundary | See dedicated stage-09 evidence/contract file; its stated limitations remain in force. |
| 10 | Window manager, input, accessibility | PARTIAL | Shell/input and bounded UI controls exist; full window management and accessibility acceptance is not complete. |
| 11 | Tokens, theming, motion | PARTIAL | `libwoven::ui::Theme`/`DesignTokens` exist; WovenSettings and WovenShell expose explicit light/high-contrast launch palettes, production system-apps boot imports the bounded installer accessibility profile into WovenShell, and `libwoven::accessibility::Settings::apply_json` is the shared fail-closed profile parser. Stage 19.14 pacing, Stage 20.7 spring animation with sustained-input coverage, Stage 20.10 Quick Settings accessibility, and desktop screenshot/input qualification pass on 1/2/4 CPUs. Settings UI write-back, persisted theme-variation screenshots, compositor-wide propagation, and low-power qualification remain open. |
| 12 | Widget framework and responsive layouts | PARTIAL | Shared bounded controls are implemented and tested on host; full responsive layout and QEMU acceptance is not complete. |
| 13 | Desktop shell and boot-to-desktop | PARTIAL | `kernel/src/shell.rs` and desktop harnesses exist; the repaired production catalog reaches the signed-in desktop and app interaction qualification on 1/2/4 CPUs. |
| 14 | VirtIO GPU 2D/QEMU | PARTIAL | `kernel/src/virtio_gpu.rs` exists; single-output, recovery and two-head QEMU gates now pass on 1/2/4 CPUs; native GPU qualification remains separate. |
| 15 | GPU acceleration foundation | UNVERIFIED | No current evidence proves a production acceleration path; CPU fallback remains the only safe assumption. |
| 16 | DOM/HTML visual runtime | PARTIAL | `libwoven::browser` now provides a bounded borrowed DOM, block/flex/grid layout, paint commands, and software-canvas rendering with host/target evidence; full browser integration and modern layout remain open. |
| 17 | JavaScript runtime and bindings | PARTIAL | `libwoven::javascript` provides a fixed-capacity scalar expression runtime and `browser::execute_script` integration with host/target evidence; full ECMAScript objects, DOM, modules, async, and sandboxing remain open. |
| 18 | CSS, Flexbox, Grid, styling | PARTIAL | `libwoven::browser` has fixed-capacity tag/class/id stylesheet rules, specificity-aware cascade with inline override, inherited text color, checked percentage lengths, bounded min/max-width media queries, flex/grid placement, paint commands, and software rendering with host/target evidence. Broader media features, full responsive semantics, and browser integration remain open. |
| 19 | HTTP/TLS fetch and network imagery | PARTIAL | `libwoven::web` parses bounded HTTP(S) endpoints/response heads and chunked bodies into caller-owned storage; sockets, TLS, policy, and network imagery integration remain open. |
| 20 | Streaming image/video/audio/codecs | PARTIAL | Bounded PNG/JPEG/BMP/GIF/WebP pixel paths and a validated PCM WAVE container parser now exist with host/target evidence. Full video/audio codec streaming and playback remain open. |
| 21 | App sandbox and graphics capabilities | PARTIAL | Existing security and graphics boundaries exist; roadmap-complete capability enforcement is not requalified. |
| 22 | Core app migration and parity | PARTIAL | Several apps use shared controls; complete migration, functionality parity, and runtime qualification are not complete. |
| 23 | Performance, stress, reliability | INCOMPLETE | Existing qualification/fault scripts exist, but the desktop baseline and 87-case software matrix pass; frame-time distributions, resource budgets and native measurements remain open. |
| 24 | Production release and documentation | INCOMPLETE | Documentation and evidence are being maintained, but release readiness cannot be claimed while mandatory gates above remain open. |

## Stage 00 gate decision

**Accepted.** The initial sign-in timeout was reproduced, diagnosed as a
headless QMP injection plus binary-frame parsing defect, corrected in the
qualification harness, and rerun successfully. The 1/2/4-CPU matrix passed
serially with the prescribed 5 GiB configuration and at least 4 GiB usable
guest memory. This acceptance covers the baseline only; it does not promote
Stages 01–24 or claim physical hardware certification.

## Safety limits

No physical disk was modified, no host reboot was performed, no unrelated QEMU
process was terminated, and no historical audit evidence was deleted. The
temporary PDF extraction file used during this audit is intentionally excluded
from the repository and is removed with this change.
