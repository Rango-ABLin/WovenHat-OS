# Stage 23.1 WovenAI broker audit

Date: 2026-10-20  
Owner: Codex  
Scope: bounded controlled-IPC WovenAI broker boundary

## Result

Accepted as the first Stage 23 increment. The dedicated QEMU gate passed on
1, 2, and 4 CPUs, each with exit code 33 and the serial marker
`[S23.1] WovenAI broker: PASSED`.

## Implemented boundary

`libwoven::ai_ipc` defines the fixed-layout `WAI1` protocol. Requests carry an
operation, correlation ID, and at most 224 bytes of validated UTF-8 input;
responses are fixed-size and return status plus value. Syscall 218 copies the
bounded request only after checking both `Capability::Ipc` and
`Capability::Ai`. The signed Ring-3 `apps/woven-ai` service publishes
`woven.ai`, returns `BROKER_ABI` for `Describe`, and returns `NoProvider` for
`Complete`. The probe is launched in the user domain with explicit `Ipc` and
`Ai` capabilities. The broker has `Ipc` only.

No model inference, provider plugin, cloud credential, file access, network
access, secret access, application control, or device access is implemented by
this stage.

## Commands and results

Run from `C:\WovenHat-OS`:

```text
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage23-1-test -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo clippy --manifest-path apps/woven-ai/Cargo.toml --target x86_64-unknown-none --release -- -D warnings
cargo test -p libwoven --lib
python -m py_compile scripts/test-stage23-1-ai-broker.py
python scripts/lint-all-features.py --only stage21
python scripts/test-stage23-1-ai-broker.py --cpus 1
python scripts/test-stage23-1-ai-broker.py --cpus 2
python scripts/test-stage23-1-ai-broker.py --cpus 4
```

The library suite passed 69 tests. The QEMU evidence is retained at:

```text
audit-artifacts/stage23.1-1cpu-1792522717245706600
audit-artifacts/stage23.1-2cpu-1792522745744098400
audit-artifacts/stage23.1-4cpu-1792522775899902400
```

## Limits and next prerequisites

The input ceiling is 224 UTF-8 bytes and the IPC transport remains bounded by
the existing message limit. Completion is intentionally unavailable until the
provider stages define local/provider ownership. Later stages must add model
resource accounting, provider isolation, credential handling, explicit data
capabilities, and separate hardware qualification. No physical USB write or
physical AI-device qualification was performed.
