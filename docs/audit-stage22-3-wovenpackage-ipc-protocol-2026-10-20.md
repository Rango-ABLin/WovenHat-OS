# WovenPackage IPC protocol foundation audit — 2026-10-20

## Implementation

`libwoven::package_ipc` adds an allocation-free, bounded `WPKI` version-1
request/response codec. Requests carry an operation (`Install`, `Remove`,
`Rollback`, or `Query`), a request ID, and a validated path. Responses carry
the request ID, status, and value. Package payload bytes remain outside the
IPC frame and must be resolved through the authenticated storage service.

Path validation rejects empty, oversized, invalid-byte, and `..` traversal
segments. Frame lengths, version, magic, and operation values fail closed.

## Validation

- `cargo test -p libwoven`: 66 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `git diff --check`: passed.

## Limits

This is a codec foundation only. No Ring-3 broker/client is wired to the
protocol yet, and no package operation is dispatched from these frames. The
next implementation must add service ownership/startup, decode-and-dispatch
with capability checks, response correlation, and 1/2/4-CPU QEMU acceptance.
No physical USB write was performed.
