# Stage 13.10R — BLE secure-session identity binding

Date: 2026-10-01

## Scope

The LE security-session authority is bound to both the controller connection
handle and the peer address identity. ATT/GATT reads, writes, notifications and
indications now reject a stale secured session after a controller reuses a
connection handle for a different peer.

The failure mode is fail-closed: a handle-only session is not sufficient to
authorize a protected operation. The live `LeLinkState` entry must match the
session address type and address. Existing disconnect and controller-reset
teardown still revoke transient sessions, while this identity check protects
against missed cleanup and handle reuse.

## Verification

The exact source tip passed:

```text
cargo build --features stage13-10-test
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none \
  --features stage13-10-test -- -D warnings
python -m unittest discover -s tests -p 'test_*.py'
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 1 ...
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 2 ...
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 4 ...
```

The host suite ran 11 tests successfully. The QEMU gate exited 33 (the
harness success code) on all three CPU counts and emitted
`[S13.10R] Bluetooth LE secure-session identity binding: PASSED`, together
with all earlier Stage 13.10 markers. Evidence was retained under
`audit-artifacts/stage13.10-*`.

This is deterministic software acceptance. Physical BLE controller/radio
interoperability, transport fault injection, SMP/LTK provisioning and hardware
qualification remain open.
