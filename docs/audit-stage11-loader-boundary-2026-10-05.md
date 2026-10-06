# Stage 11 loader boundary audit - 2026-10-05

This pass closes a production-safety hole in the Stage 11 loader boundary without
claiming full dynamic linking support.

The ELF parser now rejects `PT_INTERP`, `PT_DYNAMIC`, `PT_TLS`, and
`PT_GNU_RELRO` program headers with `Unsupported` instead of silently ignoring
them. A boot self-test mutates a known-good executable to include each header and
requires all four variants to fail closed.

This preserves the existing static executable ABI and W^X checks. It does not
implement shared libraries, dynamic relocations, TLS image allocation, or RELRO
page-protection transitions; those remain Stage 11 production work. The security
improvement is that binaries requiring those semantics can no longer be accepted
as if they were correctly loaded.

Validation:

- `cargo build`
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings`
- `cargo test`

The broader 1/2/4-CPU QEMU acceptance gate must pass before this audit entry is
treated as accepted source.
