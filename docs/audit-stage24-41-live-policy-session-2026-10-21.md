# Stage 24.41 audit - live policy-session transcript execution

Date: 2026-10-21

## Delivered

`start_policy_session` now executes the bounded `TPM2_StartAuthSession`
command through the serialized CRB transport and validates the successful
response, non-zero session handle, bounded TPM nonce, and exact response
length. `apply_policy_pcr` executes `TPM2_PolicyPCR` for one SHA-256 PCR and
expected digest, then accepts only the exact successful response framing.

Both operations are state-changing TPM transcript steps, not authorization
decisions. The session handle and initial TPM nonce are returned to the
caller; no caller-supplied packet is promoted to credential authority.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The QEMU self-test covers the existing bounded StartAuthSession and PolicyPCR
wire fixtures; serial evidence is preserved in
`audit-artifacts/stage24.41-live-policy-session-20261021`.

## Limits

`PolicyPCR` changes TPM session state, but this boundary does not yet expose
the refreshed TPM nonce after that transition or derive the quote command's
authorization HMAC. A physical TPM is not present in QEMU. Credential
decision integration, recovery, and physical qualification remain open. The
attached USB device was inventory-only and remains untouched.
