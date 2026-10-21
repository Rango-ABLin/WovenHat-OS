# Stage 24.30 audit - bounded TPM quote signature framing

Date: 2026-10-21

## Delivered

`kernel/src/tpm2.rs` now decodes bounded `TPMT_SIGNATURE` frames for SHA-256
RSASSA, RSAPSS, and ECDSA schemes. RSA signatures are bounded to 1-512 bytes;
ECDSA `r` and `s` components are bounded to 1-72 bytes. The decoder returns
the parsed shape and exact consumed length, rejecting unsupported schemes,
wrong hashes, malformed lengths, and short frames at the caller boundary.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.30-quote-signature-shape-20261021`.

## Diagnostic record and limits

The first synthetic RSA fixture declared a 256-byte TPM2B payload but used a
buffer one two-byte length field short. The fixture was corrected to the
declared 262-byte frame and the executable QEMU self-test was rerun
successfully. This stage validates wire shape only. It does not implement
RSA/ECC public-key loading, cryptographic verification, quote-key binding,
live TPM execution, recovery, or physical hardware qualification. The
attached USB device was inventory-only and remains untouched.
