# Stage 21.2 WovenPackages producer and authentication audit — 2026-10-20

## Scope

This increment adds the WPKG v1 producer and the kernel authentication
boundary. It does not install packages or mutate persistent storage.

## Implemented contract

- `tools/wpkg-packager` accepts an ELF, bounded manifest name/version,
  capability bits, bounded dependencies, and optional resource files.
- Producer output is parsed by the shared `libwoven::package` parser before it
  is written.
- Signature input is
  `SHA-256("WOVENHAT-WPKG-V1" || immutable_header_prefix || manifest ||
  application || resources)`.
- Kernel verification uses the provisioned WAPP application trust root and
  fails closed on malformed, untrusted, or tampered packages.
- Existing WAPP packaging remains unchanged.

## Validation

- `cargo test --manifest-path tools/wpkg-packager/Cargo.toml`: passed.
- `cargo clippy --manifest-path tools/wpkg-packager/Cargo.toml -- -D warnings`:
  passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-2-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Producer smoke test generated a valid 18,857-byte WPKG from the existing
  WovenCalculator ELF with capabilities and a dependency.
- Stage 21.2 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.2-1cpu-1792514011162163500`.
- Stage 21.2 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.2-2cpu-1792514024591383500`.
- Stage 21.2 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.2-4cpu-1792514038336540400`.

## Remaining acceptance prerequisites

Trust-key provisioning policy, persistent WovenStore installation, dependency
resolution, update/removal transactionality, rollback and power-loss recovery,
sandbox enforcement, service/IPC ownership, and physical USB qualification
remain unimplemented. No physical USB write was performed.
