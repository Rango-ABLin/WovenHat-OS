# Host-test boundary repair — 2026-10-14

The kernel is a freestanding `no_std`/`no_main` image. Cargo's generic
`--all-targets` Clippy pass nevertheless compiles its source in host test
contexts, so test-only compilation must not accidentally remove symbols used
by production modules or install a second panic/allocator implementation.

This increment makes that boundary explicit and testable:

- the kernel image keeps its production `no_std`, `no_main`, allocator and
  panic-handler configuration;
- host test compilation provides a test-only `PreemptMutex` implementation,
  heap-capacity no-op, and async-worker error conversion;
- test-only crate lint allowances cover code that is intentionally reachable
  only from the freestanding image;
- the kernel binary's production target remains unchanged.

Validation:

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings`
  passed;
- `cargo clippy --all-targets -- -D warnings` passed;
- `cargo test -p wovenhat-os --tests --no-fail-fast` passed across all host
  integration suites;
- `cargo test --workspace --all-targets` is not a supported host gate for this
  repository: it attempts to assemble x86-only kernel inline assembly with the
  Windows host assembler. The repository's host gate is the `wovenhat-os`
  integration-test target above; freestanding kernel behavior is validated by
  the QEMU matrices.

No production ABI or runtime behavior was changed by the test-only boundary.
