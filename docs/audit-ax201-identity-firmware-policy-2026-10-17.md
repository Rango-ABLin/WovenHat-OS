# AX201 identity and firmware-policy increment — 2026-10-17

## Scope

The selected HP Pavilion x360 reports Intel AX201 PCI identity `8086:A0F0`.
This increment adds an exact device-family record and admits the adapter to
the shared Intel 22000 activation boundary. The record associates the
integrated 22000-family firmware policy `iwlwifi-QuZ-a0-hr-b0`, API 77 only.

## Safety boundary

`classify_supported` reports the exact AX201 identity and profile. The shared
22000 CSR accessor accepts AX201, while `ax201_msi_binding_contract` validates
only the AX201 PCI ID and MSI capability. `bind_ax201_msi` is the explicit
physical MSI activation entry point. The generic AX200 binding remains
identity-specific and cannot claim AX201.

## Evidence

The subsequent driver-owner increment also passed Stage 13.10AE/AF/AG in the
Stage 13.9 1/2/4-CPU matrix. It validates and copies the API-77 runtime
image, configures queue addresses, publishes context-info through the 22000
MMIO/PRPH adapter, issues `CPU_INIT_RUN`, accepts a versioned ALIVE event, and
releases its owned DMA/context resources on stop.

The required serialized offline release validator was rerun after the owner
increment and exited 0 with `All release gates passed`.

- Final Stage 13.9 synthetic WovenWiFi acceptance passed on 1, 2 and 4 CPUs;
  retained evidence is under `audit-artifacts/stage13.9-1cpu-1792233096256524200`,
  `stage13.9-2cpu-1792233113815312000`, and
  `stage13.9-4cpu-1792233131948520100`.
- Warning-denying kernel Clippy passed with `stage13-9-test`.
- Warning-denying host all-targets Clippy passed.
- `cargo test --test wifi_tlv` passed all 8 tests.
- `git diff --check` passed.
- The Stage 13.10AD AX201 activation-contract self-test passed as part of the
  Stage 13.9 boot gate.
- The full `python scripts/test-release.py` chain was rerun against this
  increment with `CARGO_NET_OFFLINE=true` after the first attempt hit a
  transient crates.io DNS failure; it exited 0 with `All release gates passed`.

## Explicit limits

No AX201 firmware blob is embedded, and no production firmware provider or
normal-boot attachment exists yet. The owner exercises the copied-DMA and
startup protocol synthetically; physical firmware transfer/ALIVE, real DMA
ring doorbells, device interrupt delivery, teardown/recovery, scanning,
association, security, packet I/O and physical Wi-Fi acceptance remain open.
