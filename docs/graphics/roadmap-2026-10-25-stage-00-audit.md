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

## Stage ledger

| Stage | Requirement | Live classification | Evidence / gap |
|---:|---|---|---|
| 00 | Audit, baseline, evidence | **ACCEPTED** | Live repository audit is recorded here; corrected real-QEMU desktop qualification passed serially at 1/2/4 CPUs with 5 GiB configured RAM and preserved evidence. |
| 01 | Safety and graphics architecture contracts | PARTIAL | Existing kernel graphics ABI/compositor and architecture documentation exist; roadmap-specific contracts and acceptance evidence are not consolidated. |
| 02 | Display HAL and modesetting | PARTIAL | `kernel/src/graphics.rs`, `graphics_abi.rs`, and shell graphics paths exist; physical display/HAL qualification is unverified. |
| 03 | Buffered presentation and damage | PARTIAL | Compositor and framebuffer paths exist; complete damage/presentation runtime evidence is not current. |
| 04 | 2D raster/vector geometry | PARTIAL | Existing software graphics primitives are present; roadmap-complete vector/raster coverage and visual qualification are not demonstrated. |
| 05 | Fonts, Unicode, shaping, typography | PARTIAL | `kernel/src/font.rs` exists; professional shaping, fallback, and typography acceptance are not complete. |
| 06 | Iconography and asset system | PARTIAL | UI controls and app surfaces exist; complete production icon catalog and packaging acceptance are missing. |
| 07 | Images, wallpapers, asset packaging | PARTIAL | Image app and bounded image metadata parser exist; full pixel decode/asset pipeline is not implemented. |
| 08 | Secure userspace graphics surfaces and IPC | PARTIAL | Graphics ABI and userspace graphics boundaries exist; full roadmap isolation and IPC acceptance is not current. |
| 09 | Compositor and scheduler | PARTIAL | `kernel/src/compositor.rs` exists; complete scheduling, damage, latency, and stress evidence is missing. |
| 10 | Window manager, input, accessibility | PARTIAL | Shell/input and bounded UI controls exist; full window management and accessibility acceptance is not complete. |
| 11 | Tokens, theming, motion | PARTIAL | `libwoven::ui::Theme`/`DesignTokens` exist; Stage 19.14 pacing, Stage 20.7 spring animation with sustained-input coverage, Stage 20.10 Quick Settings accessibility, and desktop screenshot/input qualification pass on 1/2/4 CPUs. Theme-variation screenshots and low-power qualification remain open. |
| 12 | Widget framework and responsive layouts | PARTIAL | Shared bounded controls are implemented and tested on host; full responsive layout and QEMU acceptance is not complete. |
| 13 | Desktop shell and boot-to-desktop | PARTIAL | `kernel/src/shell.rs` and desktop harnesses exist; the repaired production catalog reaches the signed-in desktop and app interaction qualification on 1/2/4 CPUs. |
| 14 | VirtIO GPU 2D/QEMU | PARTIAL | `kernel/src/virtio_gpu.rs` exists; current end-to-end VirtIO GPU runtime acceptance is not green. |
| 15 | GPU acceleration foundation | UNVERIFIED | No current evidence proves a production acceleration path; CPU fallback remains the only safe assumption. |
| 16 | DOM/HTML visual runtime | PARTIAL | `libwoven::browser` now provides a bounded borrowed DOM, block/flex/grid layout, paint commands, and software-canvas rendering with host/target evidence; full browser integration and modern layout remain open. |
| 17 | JavaScript runtime and bindings | PARTIAL | `libwoven::javascript` provides a fixed-capacity scalar expression runtime and `browser::execute_script` integration with host/target evidence; full ECMAScript objects, DOM, modules, async, and sandboxing remain open. |
| 18 | CSS, Flexbox, Grid, styling | PARTIAL | `libwoven::browser` has fixed-capacity tag/class/id stylesheet rules, deterministic cascade with inline override, flex/grid placement, paint commands, and software rendering with host/target evidence. Selector specificity, inheritance, media queries, responsive semantics, and browser integration remain open. |
| 19 | HTTP/TLS fetch and network imagery | PARTIAL | `libwoven/src/web.rs` parses bounded HTTP(S) endpoints/response heads; sockets, TLS, policy, body streaming, and imagery integration remain open. |
| 20 | Streaming image/video/audio/codecs | PARTIAL | Bounded PNG/JPEG/BMP/GIF/WebP pixel paths and a validated PCM WAVE container parser now exist with host/target evidence. Full video/audio codec streaming and playback remain open. |
| 21 | App sandbox and graphics capabilities | PARTIAL | Existing security and graphics boundaries exist; roadmap-complete capability enforcement is not requalified. |
| 22 | Core app migration and parity | PARTIAL | Several apps use shared controls; complete migration, functionality parity, and runtime qualification are not complete. |
| 23 | Performance, stress, reliability | INCOMPLETE | Existing qualification/fault scripts exist, but the graphics roadmap matrix and current desktop baseline are not green. |
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
