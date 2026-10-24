# Stage 31 driver-restart slice — 2026-10-22

## Scope

The Stage 13.2 PCI lifecycle gate now exercises a real discovered QEMU PCI
function through bind, interrupt/BAR ownership, ordered unbind, a fresh bind
with a new owner transaction, a second teardown, and BAR-lease verification.
Generation-safe topology rescan still follows the second teardown.

## Evidence

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings` — passed.
- `python scripts/test-stage10-runtime.py --stage 13.2 --cpus 1` — passed, exit 33.
- Serial markers include `driver restart bind=true`,
  `driver restart teardown=true leases=0`, and the final Stage 13.2 pass.
- Evidence: `audit-artifacts/stage13.2-1cpu-*/serial.log`.

## Limits

This qualifies explicit PCI driver rebind/teardown and generation safety in
QEMU. It does not yet implement automatic driver-crash supervision, physical
PCI hotplug certification, or native hardware qualification.
