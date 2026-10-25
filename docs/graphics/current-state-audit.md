# WovenHat Graphics Current-State Audit

Audit date: 2026-10-25  
Audited commit: `a57f007617811792ad5d1fcb48cbe7647414b8f7`  
Branch: `master`

This is an evidence-based baseline for the graphics modernization mandate. It
does not promote any phase to complete merely because a source module exists.
Runtime claims below are limited to the recorded QEMU and host evidence.

## Executive result

The current system boots a userspace WovenShell desktop automatically in the
system-apps QEMU path. The desktop has a branded splash, procedural wallpaper,
taskbar, launcher, code-native icons, desktop shortcuts, 2D software painting,
and a 3D presentation path. The current interactive run reached the desktop
and F1 opened the launcher.

The graphics stack is not yet a complete implementation of the mandate. Window
surfaces and a kernel compositor exist, but the full production userspace
protocol, professional text/image pipeline, complete widget framework,
accelerated rendering path, native hardware qualification, and independent
security review remain incomplete or unverified. The Stage 20.1 software/QEMU
gate was subsequently closed on the 1/2/4-CPU matrix by separating its flat
fallback contract from the dedicated 3D presentation gates. Those Stage
20.17/20.18 software 3D gates are now also green on 1/2/4 CPUs after their
stale palette/start-mode fixtures were repaired.

## Source and initialization path

1. UEFI boot information supplies a framebuffer in `kernel/src/main.rs`.
2. The kernel records framebuffer address, dimensions, stride, and pixel
   format before initializing the display/compositor state.
3. `kernel/src/display.rs` provides the checked off-screen `Surface`, damage
   tracking, clipping, scaling, and framebuffer presentation path.
4. `kernel/src/compositor.rs` owns the bounded compositor surface, client blits,
   display ownership, damage presentation, and optional VirtIO-GPU outputs.
5. `kernel/src/virtio_pci.rs` provides the modern VirtIO transport and
   `kernel/src/virtio_gpu.rs` provides the bounded 2D scanout/resource path.
6. The kernel starts the userspace WovenShell session from `kernel/src/main.rs`.
7. `apps/woven-shell/src/main.rs` renders the desktop, launcher, taskbar,
   shortcuts, windows, notifications, and 3D presentation using the libwoven
   graphics interface.

## Component inventory

| Component | Status | Evidence and limitation |
|---|---|---|
| UEFI framebuffer acquisition | IMPLEMENTED | Kernel boot path reads framebuffer info and buffer. |
| Display geometry/pixel validation | IMPLEMENTED | `kernel/src/display.rs` validates dimensions, stride, bounds, and formats. |
| Off-screen surface and damage | IMPLEMENTED | Checked surface, damage rectangles, and present path are present and host-tested. |
| Double-buffered desktop composition | PARTIAL | Shell staging buffers and compositor damage exist; full frame pacing/triple buffering is not demonstrated. |
| CPU rasterization | PARTIAL | Rectangles, panels, icons, alpha/texture helpers, and shell painting exist; the mandate's complete antialiased vector API is not complete. |
| 3D presentation | PARTIAL | WovenShell has a bounded depth-tested textured path; Stage 20.17/20.18 pass on 1/2/4 CPUs, but this is not GPU acceleration or a general 3D API. |
| Typography/font engine | PARTIAL | Built-in font/text paths exist, but full TrueType/OpenType shaping, fallback, bidi, and complex-script coverage are not verified. |
| Icon/vector assets | PARTIAL | Code-native original icons are present in WovenShell; a complete reusable scalable asset library is not established. |
| PNG/JPEG/image assets | PARTIAL | Image-related application and asset paths exist; complete hardened PNG/JPEG service coverage is not verified by this audit. |
| Window surfaces | PARTIAL | Kernel client-surface registry and shell windows exist; the complete isolated production surface protocol is not closed. |
| Compositor | PARTIAL | Bounded kernel compositor supports client blits, damage, ownership, and VirtIO outputs; a dedicated userspace compositor boundary is not complete. |
| Window management | PARTIAL | Shell supports application windows, focus, close, resize-related behavior, and launcher interactions; full snapping/workspaces/modal policy is not complete. |
| WovenUI framework | PARTIAL | `libwoven::ui` now provides bounded Button/TextField/Label/Checkbox primitives with keyboard/pointer interaction helpers, theme tokens, and vertical stack layout; WovenSettings and WovenUpdate use shared controls; complete focus traversal, responsive widget coverage, and full application migration are not complete. |
| WovenDesktop shell | IMPLEMENTED for current QEMU scope | Automatic system-apps desktop boot, taskbar, launcher, clock, shortcuts, and visual shell are evidenced in QEMU. |
| Core graphical applications | PARTIAL | System app catalog includes Files, Terminal, Store, Settings, Images, Calculator, Editor, Update, Security, and Monitor; full real-service behavior for every listed feature is not verified. |
| Keyboard/mouse input | PARTIAL | QEMU keyboard/pointer acceptance exists, including launcher and desktop qualification; native hardware input remains unqualified. |
| VirtIO GPU 2D | PARTIAL | `kernel/src/virtio_gpu.rs` has scanout/resource support and QEMU gates; production hardware coverage and full reset/recovery are not closed. |
| GPU acceleration | MISSING/UNVERIFIED | No verified VirGL/Vulkan-capable execution path is claimed. CPU fallback remains the trusted path. |
| Performance qualification | UNVERIFIED | Some bounded QEMU gates exist; no complete 60-FPS/latency/memory benchmark report for the mandate exists. |
| Graphics security/fuzzing | PARTIAL | Bounds and ownership defenses exist; complete malformed image/font/protocol fuzz coverage is not complete. |
| Visual regression | PARTIAL | Guest screenshots and oracle checks exist; Stage 20.1 software/QEMU is green, while broader 3D visual coverage remains separate. |
| Native hardware certification | NOT STARTED/UNVERIFIED | Read-only physical preflights and QEMU stimulators do not constitute native qualification. |

## Current acceptance evidence

- Interactive `scripts/run-interactive-qemu.py --mode system-apps` reached the
  real desktop and the launcher opened with F1.
- WovenShell warning-denying Clippy passed for the affected target.
- Stage 20.3 launcher acceptance passed on one CPU after the icon/panel pass.
- The current release-validation records report the bounded software/QEMU
  chain, but explicitly retain open physical, production-trust, acceleration,
  and independent-review boundaries.
- Stage 20.1 now passes on 1/2/4 CPUs with the strict flat software-renderer
  contract and current branded palette.
- The WovenUI increments pass 128 libwoven host tests and Settings
  warning-denying Clippy; it is an SDK foundation, not full UI completion.
- WovenUpdate passes target build/Clippy/format checks; its app-specific QEMU
  run and the 5 GiB desktop gate both failed before sign-in in this run, so no
  new client acceptance is claimed.

## Ownership and security map

- UEFI owns initial framebuffer discovery; the kernel owns validated access and
  presentation.
- The compositor owns its bounded surface and controls display ownership.
- Client surfaces are addressed through handles and are not granted arbitrary
  global framebuffer access.
- WovenShell owns its render state in userspace and communicates with clients
  through the existing IPC/session boundary.
- VirtIO GPU resources are created through the modern PCI transport and remain
  subject to bounded protocol parsing and completion handling.
- Unsafe code remains concentrated in the kernel/low-level display and the
  shell's explicitly documented texture/render bridges; every new expansion
  must preserve ownership and teardown rules.

## Risk register

1. The current 3D shell path is a software presentation effect, not a claimed
   hardware-accelerated graphics API.
2. Text, image decoding, and reusable UI coverage are narrower than the
   mandate's full requirements.
3. Surface isolation and compositor ownership need further userspace protocol
   qualification before production claims.
4. Native GPU/input/storage and power-loss behavior require real hardware or
   explicitly scoped external qualification; QEMU cannot close those gates.
5. No independent Claude review evidence is present in this repository state.

## Immediate implementation queue

The next eligible work is to close the current graphics acceptance defect with
an evidence-preserving renderer/oracle fix, then add the smallest missing
reusable graphics abstractions and tests in roadmap order. Each increment must
retain the 1/2/4-CPU gates and remain explicitly marked partial or blocked when
the environment cannot provide hardware evidence.

