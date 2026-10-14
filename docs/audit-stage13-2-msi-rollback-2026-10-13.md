# Stage 13.2 MSI programming rollback — 2026-10-13

## Scope

The generic owner-bound MSI path now snapshots the original capability header,
message address, optional 64-bit high address, and data dword before disabling
MSI. Every message/header write is read back before the next activation step,
including the initial disabled-header write; a failed initial write is repaired
before the error is returned.
If any later configuration write or verification fails, it restores those
fields with MSI disabled during the repair and restores the original header
last. A failed repair returns `RestoreFailed`; the caller therefore retains the
interrupt activation obligation instead of recycling a vector against
uncertain device state.

The existing vector and interrupt-mode ownership checkpoints remain ordered:
software ownership is acquired before programming, and vector reuse is allowed
only after successful hardware disable and mode release.

## Evidence

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings` — passed.
- `cargo clippy -p wovenhat-os --tests -- -D warnings` — passed.
- `run-stage13-2-preservation.ps1` — passed on 1/2/4 CPUs, exit 33.
- Latest preservation evidence after the initial-header rollback hardening,
  with disposable QEMU NVMe/e1000 PCI fixtures present:
  `audit-artifacts/stage13.2-1cpu-1791894430987033100`,
  `audit-artifacts/stage13.2-2cpu-1791894442432591200`, and
  `audit-artifacts/stage13.2-4cpu-1791894453903648700`.

## Limits

The QEMU preservation device did not provide a valid directly mappable BAR for
physical MSI/MSI-X activation. This change proves rollback behavior and keeps
physical interrupt-controller qualification open.
