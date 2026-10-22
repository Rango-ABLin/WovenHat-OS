# Stage 21.8 WovenPackage install-intent recovery audit — 2026-10-20

## Scope

This increment adds a bounded, checksummed WPKJ install intent around the
Stage 21.7 payload/catalog publication path. The gate simulates an interrupted
prepared install and validates orphan cleanup on a disposable ATA image.

## Implemented contract

- `Prepared` intent is durable before payload chunks are written.
- `Committed` intent is written after catalog publication.
- Recovery validates the journal checksum and package identity.
- Prepared intents without a matching catalog entry remove their application
  and resource chunks before the journal is cleared.
- Committed intents represented by the catalog are retained and cleared.
- A malformed journal fails closed and is cleared rather than trusted.

## Validation

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-8-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.8 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.8-1cpu-1792515508984877300`.
- Stage 21.8 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.8-2cpu-1792515523112811000`.
- Stage 21.8 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.8-4cpu-1792515537897023500`.

## Remaining acceptance prerequisites

Multi-package journal recovery, persistent update/removal transactions,
power-loss injection at every publication boundary, userspace package-service
IPC/ownership, and physical USB qualification remain unimplemented. No
physical USB write was performed.
