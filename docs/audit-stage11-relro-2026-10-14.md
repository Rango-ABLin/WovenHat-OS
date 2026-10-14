# Stage 11 loader RELRO audit — 2026-10-14

## Scope

The ELF loader now accepts a single `PT_GNU_RELRO` range only when it is
page-aligned into a non-executable writable load segment. After the image is
copied and before the process is published, the loader changes that range to
read-only. Address-space write-fault bookkeeping carries the same range, so a
fork/COW fault cannot re-enable writes to RELRO pages. Relocation by the
production ASLR delta updates the RELRO range with the load segments.

`PT_DYNAMIC` and `PT_INTERP` remain rejected. Static `PT_TLS` is now supported
by the separate initial-exec TLS path documented in the
[static TLS audit](audit-stage11-static-tls-2026-10-14.md). Dynamic symbol
resolution, shared-library ownership, TLS module IDs, and RELRO relocation
sequencing remain outside the loader contract.

## Validation

- Valid and invalid RELRO parser cases are covered by the loader self-test.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage11-5-test -- -D warnings` — passed.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `cargo test -p wovenhat-os --tests --no-fail-fast` — passed.
- Release build and `git diff --check` — passed.
- Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs, exit 33:
  `audit-artifacts/stage11.5-1cpu-1791971762993756000`,
  `audit-artifacts/stage11.5-2cpu-1791971862705300300`,
  `audit-artifacts/stage11.5-4cpu-1791971961472267000`.

## Limits

The acceptance image uses the existing static ELF fixtures; it does not claim
dynamic linking, shared libraries, or physical hardware enforcement of page
permissions. Those remain explicit follow-up requirements.
