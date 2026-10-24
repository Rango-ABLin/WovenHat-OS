# Stage 25.2 audit - CPU idle capability boundary

Date: 2026-10-21

## Scope

This increment adds a read-only CPUID-derived CPU idle capability boundary.
Syscall 236 and `libwoven::cpu_idle_capabilities` report three bytes for
architectural `HLT`, optional `MONITOR/MWAIT`, and invariant-TSC support. The
query requires `PowerControl`, records an audit event, validates the user
buffer length and copy, and changes no CPU state. The scheduler's existing
interrupt-safe `STI+HLT` path is unchanged.

## Verification

- Signed production-kernel Clippy with `system-apps` and `-D warnings`: pass.
- `libwoven` all-targets Clippy with `-D warnings`: pass.
- `libwoven` host tests: pass, 95 tests.
- Workspace all-targets host Clippy with `-D warnings`: pass.
- Full workspace `cargo test --all`: pass, including 95 `libwoven` tests and
  all repository integration suites.
- Signed system-apps QEMU shell acceptance: pass on 1 CPU, 2 CPUs, and 4
  CPUs; release shell, PS/2/IOAPIC input, and SMP smoke markers remained
  green.
- `git diff --check`: pass before handoff.

## Limits and next prerequisites

This is not an idle-policy implementation. It does not execute MWAIT, write
frequency-control MSRs, select deep C-states, account residency per CPU, or
implement suspend/resume. Those features require per-CPU coordination,
interrupt/wakeup proofs, hardware-model policy, and physical qualification.
