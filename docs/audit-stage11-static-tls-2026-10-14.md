# Stage 11 static TLS audit — 2026-10-14

## Scope

The ELF loader now accepts one validated `PT_TLS` image in an otherwise
static executable. It allocates a page-aligned, writable initial TLS block
below the user stack guard, zero-fills it, copies the file image, and exposes
the block end as the task's FS base. The FS base is installed on first entry,
exec, and every scheduler context switch. Fork shares the TLS pages through
the existing copy-on-write address-space path, and TLS writes are treated as
writable for COW fault bookkeeping.

The parser rejects duplicate or malformed TLS headers, including zero-sized,
file-larger-than-memory, non-power-of-two, over-page alignment, and truncated
file ranges. `PT_DYNAMIC` and `PT_INTERP` remain rejected. Dynamic relocation,
shared-library TLS, TLS module IDs, and multiple kernel-managed user threads
remain outside this static initial-exec contract.

## Validation

- Static and invalid TLS parser fixtures pass the loader self-test.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage11-5-test -- -D warnings` — passed.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `cargo test -p wovenhat-os --tests --no-fail-fast` — passed.
- Stage 11.5 QEMU acceptance passed on 1, 2, and 4 CPUs:
  - `audit-artifacts/stage11.5-1cpu-1791972922171894100`
  - `audit-artifacts/stage11.5-2cpu-1791972980542087100`
  - `audit-artifacts/stage11.5-4cpu-1791973031110997200`

These gates validate the loader and scheduler contract in QEMU; they do not
claim physical hardware TLS qualification or dynamic-linker support.
