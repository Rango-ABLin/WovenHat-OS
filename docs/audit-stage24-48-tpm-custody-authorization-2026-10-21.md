# Stage 24.48 audit - TPM custody authorization boundary

Date: 2026-10-21

## Delivered

TPM policy-session custody now has a kernel-owned `PolicySessionCustodyAuthority`
token with private construction and a monotonic generation. Issuance requires
a validated aligned CRB descriptor and, in system-apps release boots, a
successfully retained signed TAN1 policy. Both `persist_policy_session_snapshot`
and `load_policy_session_snapshot` require the token in addition to the native
WovenFS writer/read boundary. A storage-only caller cannot create or recover
TPM credential-session custody.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same system-apps hardware acceptance with 2 and 4 CPUs: passed.

The QEMU TPM self-test covers aligned-descriptor issuance and invalid
descriptor rejection. Serial evidence is preserved in
`audit-artifacts/stage24.48-tpm-custody-authorization-20261021`.

## Limits

QEMU has no live TPM and therefore cannot prove physical policy loading,
cross-reboot context restore, or TPM-backed credential use. Provisioned-key
lifecycle, credential decisions, recovery policy, and physical qualification
remain open. The attached USB device was not modified.
