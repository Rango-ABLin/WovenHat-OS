# Stage 12.3 volume encryption — 2026-09-16

Stage 12.3 now uses an RFC 8439 ChaCha20-Poly1305 implementation in the
freestanding kernel. Encryption and decryption operate on caller-owned buffers,
produce and verify a detached 128-bit tag, authenticate associated metadata,
and compare tags in constant time. The ChaCha20 core uses bounded 32-bit
arithmetic so it builds on `x86_64-unknown-none` without architecture-specific
SIMD code-generation failures.

The bounded key vault holds eight 256-bit keys. Provisioning rejects an all-zero
key, records the owning identity, returns a generation-safe opaque handle, and
revocation erases the slot. Owner-scoped operations reject cross-identity use;
stale handles cannot address a subsequently provisioned key. The structural
probe covers ciphertext mutation, tag mutation, associated-data mutation,
successful round trips, cross-owner rejection, revocation, and stale-handle
rejection.

Evidence retained on 2026-09-16:

- `cargo build -p wovenhat-kernel --target x86_64-unknown-none` — PASS.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage12-3-test -- -D warnings` — PASS.
- `python scripts/test-stage10-runtime.py --stage 12.3 --cpus 1` — PASS.
- `python scripts/test-stage10-runtime.py --stage 12.3 --cpus 2` — PASS.
- `python scripts/test-stage10-runtime.py --stage 12.3 --cpus 4` — PASS.
- Full `python scripts/test-release.py` after this implementation — PASS; all
  lint, host, 1/2/4-CPU, release, and shell/SMP gates passed.

## Production-completion pass — 2026-09-29

Stage 12.3 now adds authenticated persistent wrapped-key records whose associated
data binds volume identity and generation; an explicit measured/authenticated
provisioning-evidence gate; successor-only, overflow-checked rotation plans that
retain the old generation until durable metadata advancement; and encrypted
mount-record I/O that retains only opaque vault authority and authenticates the
volume, generation, and mount identity.

The adversarial gate rejects untrusted measurement evidence, tampered wrapped
records, cross-volume or skipped-generation rotation, tampered mounted-record
tags, and revoked/stale mount authority. Authentication failure leaves mounted
ciphertext untouched.

GitHub Actions run 614 (run id 36577792537) passed release validation for commit
`54c7dd938faad567c97de74fb45ec0d2d64286c5`, including lint, host regressions,
the 1/2/4-core boot matrix, and validation-evidence preservation.

Stage 12.3 is production-complete at the kernel integration boundary. Physical
hardware still needs a TPM/firmware measured-root backend to produce the trusted
evidence consumed by this boundary; best-effort kernel entropy is explicitly not
accepted as a substitute for that hardware trust source.
