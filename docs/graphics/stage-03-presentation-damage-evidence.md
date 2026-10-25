# Buffered presentation and damage tracking — Stage 03 evidence

Status: **PARTIAL / SUB-GATE GREEN**  
Date: 2026-10-25

`kernel/src/display.rs` provides bounded off-screen surfaces, checked geometry,
damage regions, stride-safe framebuffer conversion, and damage-only
presentation. `kernel/src/compositor.rs` clears damage only after successful
copy and routes GPU and UEFI framebuffer paths through the same contract.
`kernel/src/user_graphics.rs` enforces owner/generation and pixel budgets.

The real QEMU hello-gui damage compositor acceptance passed serially:

- 1 CPU: `target/hello-gui-damage-1cpu`
- 2 CPU: `target/hello-gui-damage-2cpu`
- 4 CPU: `target/hello-gui-damage-4cpu`

Each run verified damage rules, unprivileged/foreign-surface refusal, peer
mailbox registration, compositor sleep-on-damage, damage-only copy,
undeclared-region protection, clean exits, and lease release. The full desktop
qualification also passed at 1/2/4 CPUs in the Stage 00 evidence directories.

The ownership, bounds, damage, and SMP sub-gates are green. Stage 03 remains
**partial** until dedicated frame-pacing, repeated-buffer-reuse, overflow, and
flicker/stress evidence is recorded. No triple-buffering or physical scanout
guarantee is claimed.

