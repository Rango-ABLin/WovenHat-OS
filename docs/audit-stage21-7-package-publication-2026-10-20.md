# Stage 21.7 authenticated WovenPackage publication audit — 2026-10-20

## Scope

This increment adds the first payload publication path above the authenticated
WPKG verifier and native WovenFS catalog persistence. It validates one package
with a small ELF payload on a disposable ATA image.

## Implemented contract

- The service accepts a `Package` returned by the kernel WPKG verifier.
- Application and resource data are split into bounded native-WovenFS chunks.
- Chunk paths include package name and version, preserving the old generation
  during an update staging attempt.
- The WSTC catalog is committed only after all payload chunks are durable.
- Chunk and catalog failures clean up chunks created by the attempted install.
- The service's writer token is capability-gated through `StorageIo`.

## Validation

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-7-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.7 first boot attempt was rejected by the harness because an earlier
  Stage 15.2 exit block ran before the package marker; evidence is preserved at
  `audit-artifacts/stage21.7-1cpu-1792515202218981500`. The guard was corrected
  and the gate rerun.
- Stage 21.7 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.7-1cpu-1792515241512085800`.
- Stage 21.7 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.7-2cpu-1792515253821407400`.
- Stage 21.7 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.7-4cpu-1792515268155840600`.

## Remaining acceptance prerequisites

Multi-package transactional recovery, update/removal persistence, reboot and
power-loss injection, userspace package-service IPC/ownership, and physical
USB qualification remain unimplemented. No physical USB write was performed.
