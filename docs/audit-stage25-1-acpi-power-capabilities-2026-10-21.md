# Stage 25.1 audit - ACPI power capability boundary

Date: 2026-10-21

## Scope

This increment adds a bounded, read-only power-management boundary. ACPI
discovery recognizes `_S3` sleep types in addition to the existing `_S5`
power-off types. Syscall 235 and `libwoven::power_capabilities` report three
capability bytes: power-off, suspend, and restart. The query requires the
`PowerControl` capability, records an audit event, validates the requested
userspace output length, and performs no transition.

The always-on ACPI structural self-test now includes a bounded synthetic AML
package covering `_S3` discovery, rejection of an unrelated `_S5` lookup, and
rejection of unsupported sleep-state selectors.

## Verification

- Signed production kernel Clippy with `system-apps` and `-D warnings`: pass.
- `libwoven` all-targets Clippy with `-D warnings`: pass.
- `libwoven` host tests: pass, 95 tests.
- Signed system-apps QEMU shell acceptance: pass on 1 CPU, 2 CPUs, and 4
  CPUs; release shell, PS/2/IOAPIC input, and SMP smoke markers remained
  green.
- The signed 1/2/4-CPU boots exercised the ACPI structural self-test, including
  the new `_S3` AML fixture.
- The source change was compiled into the signed production image used by
  those QEMU runs.
- `git diff --check`: run as part of the stage handoff.

## Limits and next prerequisites

This is not suspend/resume implementation. It does not save or restore driver
state, quiesce devices, select wake sources, expose battery/thermal/lid
telemetry, or qualify real firmware and hardware. A later Stage 25 increment
must add an event-driven transition state machine and per-driver ownership and
rollback rules before any S3 entry is exposed. Physical TPM/USB qualification
also remains open.
