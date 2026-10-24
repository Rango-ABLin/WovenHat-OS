# Stage 24.42 audit - policy-session quote HMAC derivation

Date: 2026-10-21

## Delivered

`derive_quote_session_hmac` constructs the exact SHA-256 cpHash input for
`TPM2_Quote` (command code, signing handle, qualifying data, signature
scheme, and PCR selection), then computes the authorization HMAC over cpHash,
the returned TPM session nonce, caller nonce, and session attributes using the
caller-supplied attestation-key auth value. The bounded wrapper
`quote_with_derived_policy_hmac` passes the resulting 32-byte HMAC through
the non-password quote path. The password-session handle is rejected there.

No key material is read from a TPM response, and no fallback HMAC is
fabricated. The existing quote identity, nonce/PCR, and cryptographic
verification boundary remains mandatory.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The QEMU self-test covers deterministic HMAC derivation and detects an
auth-value change; serial evidence is preserved in
`audit-artifacts/stage24.42-policy-session-hmac-20261021`.

## Limits

The derivation currently supports only the null-salt/null-bind session shape
created by this kernel. It does not yet implement provisioned key lifecycle,
session-state refresh after every TPM transition, credential decision
integration, recovery, or physical TPM qualification. The attached USB
device was inventory-only and remains untouched.
