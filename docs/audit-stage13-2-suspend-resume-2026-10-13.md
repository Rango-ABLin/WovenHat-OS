# Stage 13.2 PCI suspend/resume audit — 2026-10-13

Owned PCI suspend now validates the generation owner, disables I/O, memory,
and bus-master command decoding, and verifies the result. When firmware exposes
the PCI Power Management capability, it also checkpoints PMCSR and enters
D3hot with read-back verification. Resume validates ownership, restores D0,
then restores the exact command checkpoint with read-back verification. Any
failed transition remains fail-closed and attempts the safe rollback state.
PMCSR writes preserve the upper half of the containing configuration dword so
adjacent PM capability fields are not overwritten.
The driver manager uses this path only for PCI bindings; the established
non-PCI state transition is preserved. Missing checkpoints fail closed instead
of treating a PCI binding as a generic driver.

Validation completed:

- freestanding and host warning-denying Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791889555695876300`,
  `audit-artifacts/stage13.2-2cpu-1791889566832554400`, and
  `audit-artifacts/stage13.2-4cpu-1791889577521462400`.

This validates the software lifecycle and the PMCSR transition when a valid
capability is present. Device-specific reset, D3cold, platform wake signaling,
and physical suspend/resume behavior remain hardware and driver dependent.
