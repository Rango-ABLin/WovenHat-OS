# Stage 21.5 WovenStore durable record codec audit — 2026-10-20

## Scope

This increment defines the bounded `WSTC` v1 catalog record used by a future
package service for durable WovenFS publication. It does not perform storage
I/O or claim reboot durability.

## Implemented contract

- Fixed maximum record size and 32-entry catalog capacity.
- Magic, version, exact length, capability policy, entry metadata, and
  dependency fields.
- Checksum over the immutable header and entry bytes.
- Decode rejection for corruption, malformed lengths, invalid names, duplicate
  entries, and invalid dependency fields.
- Host round-trip and checksum-corruption tests.

## Validation

- `cargo test -p libwoven`: passed; 63 tests.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.5 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.5-1cpu-1792514640957039400`.
- Stage 21.5 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.5-2cpu-1792514655302555900`.
- Stage 21.5 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.5-4cpu-1792514670539406700`.

## Remaining acceptance prerequisites

Native WovenFS write/read integration, power-loss recovery, package service
authority/IPC, persistent payload publication, and physical USB qualification
remain unimplemented. No physical USB write was performed.
