# Stage 24.39 audit - bounded TPM2 Quote execution

Date: 2026-10-21

## Delivered

The TPM2 CRB layer now exposes `tpm2::quote`, which builds a bounded
sessions-tagged `TPM2_Quote` request for one signing handle, a qualifying
nonce of at most 32 bytes, one SHA-256 PCR, and an explicit RSA-SSA,
RSA-PSS, or ECDSA scheme. The command uses an empty password session only for
wire authorization framing; it does not claim to authorize a non-empty object
policy.

The response parser requires the sessions response tag, exact response size,
successful TPM return code, bounded `TPMS_ATTEST` and `TPMT_SIGNATURE`
parameters, supported signature algorithms, and an exact empty password
authorization response. Quote and signature bytes are returned in fixed-size
kernel-owned buffers. They remain evidence and cannot create authority until
the existing `authorize_quote` path verifies the signed Name, ReadPublic
binding, nonce/PCR digest, and cryptographic signature.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The executable QEMU self-test covers command sizing, handle/session fields,
nonce, scheme, PCR selection count/bitmap, and bounded synthetic response
decoding. Serial evidence is preserved in
`audit-artifacts/stage24.39-tpm-quote-execution-20261021`.

## Limits

The QEMU environment has no physical TPM, so it cannot prove a successful
live quote. The current API intentionally supports only an empty password
session; production attestation keys requiring non-empty authorization need a
subsequent policy/session-authenticated quote path. Credential decision
integration, recovery, and physical TPM qualification remain open. The
attached USB device was inventory-only and remains untouched.
