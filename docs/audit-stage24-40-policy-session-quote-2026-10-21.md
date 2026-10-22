# Stage 24.40 audit - policy-session TPM2 Quote framing

Date: 2026-10-21

## Delivered

`QuoteSessionAuth` extends the bounded quote path with an explicit
non-password session handle, caller nonce, session attributes, and
authorization HMAC. `quote_with_policy_session` rejects the password-session
handle and passes the caller-supplied authorization material unchanged into
the TPM sessions-tagged command. The encoder bounds nonce and HMAC sizes and
places the authorization area before quote parameters as required by the TPM
command layout.

The API does not fabricate an HMAC or claim to create a policy session. The
existing `authorize_quote` path remains mandatory after the TPM returns quote
evidence.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The executable QEMU self-test covers empty-password and policy-session quote
command offsets, session sizes, caller nonce, attributes, HMAC, PCR
selection, and downgrade rejection. Serial evidence is preserved in
`audit-artifacts/stage24.40-policy-session-quote-20261021`.

## Limits

This stage does not yet start the policy session, execute `PolicyPCR`, or
derive the authorization HMAC from provisioned TPM/session material. It also
does not perform a live physical TPM quote in this environment. Credential
decision integration, recovery, and physical qualification remain open. The
attached USB device was inventory-only and remains untouched.
