# Stage 27.5 update-service audit — 2026-10-21

## Implemented

- Added the capability-gated kernel transaction boundary in
  `kernel/src/update_service.rs`.
- Added syscall 239 and the safe `libwoven::update_transaction` wrapper.
- Verified the signed WUPD envelope before any state transition.
- Wrote the inactive slot before committing the next dual-copy WUAB record.
- Added sequence arbitration, missing-copy recovery, and fail-closed invalid
  or tied-copy handling.
- Implemented bounded stage, activate, boot-failed, boot-succeeded, and query
  operations.
- Added `scripts/test-stage27-5-update-service.py`.

## Evidence

Commands were run serially in the project-local target directory:

```text
cargo clippy --offline -p wovenhat-kernel --target x86_64-unknown-none --features stage27-5-test -- -D warnings  PASS
cargo clippy --offline -p libwoven --all-targets -- -D warnings                                      PASS
cargo test --offline -p libwoven                                                                        113 passed
python scripts/test-stage27-5-update-service.py --cpus 1                                                PASS
python scripts/test-stage27-5-update-service.py --cpus 2                                                PASS
python scripts/test-stage27-5-update-service.py --cpus 4                                                PASS
```

Retained QEMU evidence is under `audit-artifacts/stage27.5-{1,2,4}cpu/`.

## Limits and next prerequisites

The current WovenFS implementation limits one durable file to 4 KiB. The
service rejects larger images with `ImageTooLarge`; it does not split or
silently truncate them. This stage therefore proves the transaction and
authority boundary, not production image installation. Remaining work before
release includes the long-lived privileged WovenUpdate daemon and WUDI IPC
loop, a block-device/resumable slot backend for production image sizes,
bootloader handoff and persisted boot-attempt decisions, power-loss/fault
injection recovery, release-key operations, and destructive physical USB
qualification under an explicitly approved device identity.
