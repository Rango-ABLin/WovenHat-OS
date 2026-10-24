# Stage 24.44 audit - TPM policy-session context save/load

Date: 2026-10-21

## Delivered

The policy-session lifecycle now includes bounded `TPM2_ContextSave` and
`TPM2_ContextLoad` framing. Context responses require the no-sessions tag,
exact response size, successful TPM code, non-zero saved handle, and a
non-empty context blob no larger than the fixed kernel bound. Context loads
validate the exact 14-byte handle response.

Successful save invalidates the active lease and returns a snapshot containing
the validated context and session metadata. A snapshot is consumed only after
a successful load, which returns a fresh generation; failed loads remain
available for retry.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The QEMU self-test covers exact context save/load framing, blob bounds, and
restored-handle decoding. Serial evidence is preserved in
`audit-artifacts/stage24.44-policy-session-context-20261021`.

## Limits

This stage does not establish secure cross-reboot context custody or refresh
session nonce state after every TPM transition. Provisioned-key lifecycle,
credential decisions, recovery, and physical TPM qualification remain open.
The attached USB device was inventory-only and remains untouched.
