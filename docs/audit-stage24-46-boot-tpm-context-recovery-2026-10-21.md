# Stage 24.46 audit - boot-time TPM context recovery

Date: 2026-10-21

## Delivered

System-apps boot now retains the discovered TPM2 handle across ACPI reporting.
After WovenFS mount and signed TAN1 policy loading, it loads the newest
validated `TPS1` custody candidate, requires TPM2 `ContextLoad`, and flushes
the restored policy-session lease immediately. The current authority consumer
is intentionally absent, so successful recovery is still reported as
hardware-credentials-disabled. Every absence, storage error, malformed record,
TPM rejection, and cleanup failure remains fail-closed.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same system-apps hardware acceptance with 2 and 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.46-boot-tpm-context-recovery-20261021`.

## Limits

QEMU has no durable TPM context, so a successful physical cross-reboot
`ContextLoad` is not proven. Secure storage authorization, session nonce
refresh, provisioned-key lifecycle, credential decisions, recovery policy, and
physical qualification remain open. The attached USB device was not written
or otherwise modified.
