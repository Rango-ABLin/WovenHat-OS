# Graphics architecture contracts — Stage 01

Status: **accepted for the current implementation boundary**  
Date: 2026-10-25  
Prerequisite: Stage 00 accepted in commit `4a194cc7`

This contract maps the graphics roadmap names to the existing WovenHat source
boundaries. It is intentionally an adapter document: existing accepted kernel,
compositor, surface, input, IPC, security, and shell implementations remain
the implementation of record.

## Ownership and boundaries

| Boundary | Owner | Current implementation | Authority |
|---|---|---|---|
| WovenDisplay | Kernel display/terminal layer | `kernel/src/terminal.rs`, `kernel/src/display.rs`, `kernel/src/virtio_gpu.rs` | Kernel owns framebuffer mapping, stride/format metadata, display ownership, and fallback scanout. |
| WovenRender | Userspace CPU renderer | `libwoven/src/graphics.rs`, `libwoven/src/lib.rs` `Canvas`/surface helpers | Caller-owned buffers; no renderer input is trusted as a kernel pointer. CPU rendering remains the mandatory fallback. |
| WovenText | Kernel/shared font plus userspace metrics | `kernel/src/font.rs`, `libwoven/src/font.rs`, `libwoven/src/graphics.rs` | Font bytes are borrowed/validated; shaping remains a later stage and is not implied by this contract. |
| WovenAssets | Userspace/application asset boundary | Existing app resources and bounded `libwoven` parsers | Assets are immutable caller-owned data; decoders must enforce dimensions, lengths, and allocation budgets before Stage 07/20 exposure. |
| WovenCompositor | Kernel lease and userspace shell policy | `kernel/src/compositor.rs`, `kernel/src/user_graphics.rs`, `apps/woven-shell/src/main.rs` | Kernel owns single-holder lease, surface ownership/generation, damage/presentation authority; shell owns stacking policy. |
| WovenWindow | Userspace shell | `apps/woven-shell/src/main.rs`, `libwoven/src/layout.rs` | Shell owns window state, z-order, workspaces, focus policy, and client lifecycle; clients never acquire the compositor lease. |
| WovenUI | Userspace shared controls | `libwoven/src/ui.rs` and migrated system apps | Controls are bounded, caller-owned, and event-driven. They do not gain kernel or filesystem authority. |
| WovenDesktop | Userspace boot session | `apps/woven-shell/src/main.rs`, `kernel/src/session.rs` | Kernel starts the signed session with explicit capabilities; shell owns desktop presentation and recovery remains F12/diagnostic-console based. |

## Dependency graph

```text
UEFI framebuffer / VirtIO-GPU probe
              |
              v
        WovenDisplay (kernel)
              |
              v
  compositor lease + owned surfaces + damage IPC
       |                    |
       v                    v
 WovenCompositor       WovenWindow/Desktop
       |                    |
       +----------+---------+
                  v
        WovenRender / WovenText / WovenAssets
                  |
                  v
             WovenUI + apps
```

The kernel path is deliberately below untrusted document, asset, codec, and
JavaScript paths. Future web/media stages may consume the userspace renderer,
but cannot gain privileged memory, display, storage, or input authority.

## Contract invariants

1. **Display fallback:** when VirtIO-GPU is unavailable or rejects a scanout,
   the UEFI framebuffer path remains available. No accelerated path may remove
   or bypass the CPU fallback.
2. **Lease ownership:** only the process holding the compositor lease may
   mutate or present the compositor surface. Client surface handles are
   owner- and generation-checked by the kernel.
3. **Input authority:** the presenting lease holder receives the bounded focus
   queue. Input is not broadcast to clients, and F12 remains a kernel/session
   recovery boundary.
4. **Damage discipline:** clients declare damage; the compositor copies bounded
   regions and may conservatively recompose. A failed client or stale handle is
   removed/rejected, never read through.
5. **Pointer and async lifetime:** userspace pointers are copied/validated by
   syscall boundaries before asynchronous use. No surface, event, or asset
   operation stores an unvalidated userspace pointer.
6. **Blocking:** shell and clients wait on input/message/damage events. No task
   sleeps while holding an IRQ lock, compositor lock, or surface registry lock.
7. **Capability separation:** `DisplayIo`, `InputIo`, and `Ipc` are explicit
   capabilities. UI controls and future web/media runtimes receive no implicit
   kernel capability.
8. **Bounded resources:** surface dimensions, per-process/system pixel budgets,
   event queues, parser limits, and asset dimensions are fail-closed.

## Migration and rollback

The migration order is additive: display diagnostics → presentation/damage →
render/text/assets → compositor/window/input → UI/desktop → web/media/apps.
Each increment keeps the current UEFI framebuffer and software renderer path
available. A failed new path is rolled back by disabling its feature or
releasing its lease; it does not require disk mutation or a host reboot.

The Stage 00 QEMU qualification is the rollback sentinel. After each graphics
increment, run host tests, warning-denying target checks, and the desktop
qualification at 1/2/4 CPUs before promoting the increment. If a matrix entry
fails, retain its evidence, revert only the unaccepted increment, and preserve
the last accepted commit as the recovery point.

## Stage 01 review evidence

- Stage 00 baseline: commit `4a194cc7`; real-QEMU desktop qualification passed
  at 1/2/4 CPUs with 5 GiB configured RAM and a 4 GiB usable-memory floor.
- Existing compositor/surface/input boundaries were reviewed in
  `kernel/src/compositor.rs`, `kernel/src/user_graphics.rs`,
  `kernel/src/woven_input.rs`, `kernel/src/syscall.rs`, and
  `apps/woven-shell/src/main.rs`.
- The accepted host graphics/UI suite remains `cargo test -p libwoven --quiet`;
  target warning-denying Clippy remains required for affected applications.
- This stage does not claim modern shaping, CSS, ECMAScript, streaming codecs,
  GPU acceleration, or physical hardware certification. Those are later gates.

