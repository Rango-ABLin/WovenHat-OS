# Stage 24.49 audit - credential-root lifecycle

Date: 2026-10-21

## Delivered

`CredentialRootLease` is a kernel-owned, non-copyable lifecycle object for a
32-byte root that has already passed TPM NV metadata and exact readback
validation. Installation rejects zero roots and assigns a monotonic
generation. Rotation validates the replacement first, rejects same-root
replays, wipes the old root, and advances the generation. Derivation exposes
only a domain-separated HMAC result; the root is never returned. Explicit
revocation and `Drop` wipe the root and prevent further derivation.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same system-apps hardware acceptance with 2 and 4 CPUs: passed.

The QEMU TPM self-test covers zero-root refusal, installation, derivation,
generation-changing rotation, and post-revocation refusal. Serial evidence is
preserved in `audit-artifacts/stage24.49-credential-root-lifecycle-20261021`.

## Limits

QEMU does not provide a live TPM, so NV define/write/read execution and
physical provisioned-root persistence are not proven. Credential decision
integration, reboot recovery policy, and physical qualification remain open.
The attached USB device was not modified.
