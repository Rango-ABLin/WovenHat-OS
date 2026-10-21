# Stage 25.3 audit - CPU frequency inventory boundary

Date: 2026-10-21

## Scope

This increment adds a read-only CPUID leaf `0x16` frequency inventory path.
Syscall 237 and `libwoven::cpu_frequency_info` return an availability byte
and bounded base, maximum, and bus MHz values. The query requires
`PowerControl`, records an audit event, validates the 13-byte userspace buffer,
and performs no P-state or MSR write.

## Verification

- Signed production-kernel Clippy with `system-apps` and `-D warnings`: pass.
- `libwoven` all-targets Clippy with `-D warnings`: pass.
- `libwoven` host tests: pass, 95 tests.
- Signed system-apps QEMU shell acceptance: pass on 1 CPU, 2 CPUs, and 4
  CPUs; release shell, PS/2/IOAPIC input, and SMP smoke markers remained
  green.
- `git diff --check`: pass before handoff.

## Limits and next prerequisites

This does not implement frequency scaling, a governor, thermal throttling,
per-CPU policy, or physical frequency measurement. Those require validated
hardware-specific controls, rollback and interrupt coordination, thermal
policy, and physical qualification.
