# Stage 13.10S closure — BLE minimum key size on outbound GATT

Date: 2026-10-05

The Stage 13.10S minimum encryption-key-size policy now applies consistently
to protected ATT reads/writes and to secured notification/indication emission.
A live authenticated session with a key below the policy threshold is rejected
before an outbound value is encoded. A session at the configured threshold is
allowed only after the identity-bound session lookup succeeds.

Verification on the exact source tip:

```text
cargo build --features stage13-10-test
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none \
  --features stage13-10-test -- -D warnings
python -m unittest discover -s tests -p 'test_*.py'  # 11 passed
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 1 ...  # PASS
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 2 ...  # PASS
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 4 ...  # PASS
```

The QEMU harness exited with its expected success code 33 and retained serial
evidence under `audit-artifacts/stage13.10-*`. This remains deterministic
software validation; physical BLE controller/radio interoperability, SMP/LTK
provisioning and hardware qualification remain open.
