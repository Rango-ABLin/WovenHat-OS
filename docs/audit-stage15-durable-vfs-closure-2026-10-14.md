# Stage 15 durable VFS closure audit — 2026-10-14

## Scope and verified integration

This review examines the current `master` source, rather than assuming that a merged pull request implies release acceptance.

- `kernel/src/task.rs::open_current` checks `FileRead` and path authority; creating a missing `/wfs/` file additionally checks `FileWrite` and path authority, persists it via `wfs::ensure_vfs_path`, then creates a read-only VFS cache entry.
- `kernel/src/task.rs::write_current` checks `FileWrite` and scope, dispatches `/wfs/` writes to `wfs::write_vfs_path_at`, and publishes the returned image via `vfs::commit_backend_write`.
- `kernel/src/task.rs::unlink_current` checks `FileWrite` and path authority and unlinks the native file before removing the VFS node.
- `kernel/src/wfs.rs::write_vfs_path_at` serializes native mutations with `MUTATION`, writes the complete file image, and returns success only after native write completion.
- `kernel/src/vfs.rs::commit_backend_write` checks descriptor offset and generation, updates the VFS image and file-frame cache, and leaves generic VFS writes disabled for the native-backed node.

**Correction to earlier milestone notes:** ordinary task-level file-write dispatch is already present on `master`. The remaining work is *concurrency, failure and reboot qualification*, not first-time syscall wiring.

## Closure risks requiring proof

1. **Durable-write/cache-publication split.** The native transaction commits before `commit_backend_write` verifies descriptor offset and generation. If another writer moves the descriptor or invalidates the node between these operations, a syscall may report failure although the durable bytes changed. Test concurrent writers and define serialization or reconciliation semantics.
2. **Native/VFS multiwriter ordering.** Native writes are protected by `wfs::MUTATION`, but the VFS publication happens after releasing that lock. Two successful native writes could publish cached images out of order. Test interleavings with separate descriptors and compare on-disk, VFS and file-frame contents.
3. **Crash consistency.** Simulate interruption at each journal/checkpoint boundary, remount and check that the file is entirely old or entirely new; repeat on physical media with controlled power-loss testing.
4. **Open-handle invalidation.** Test unlink/recreate while descriptors and memory mappings exist, then check generation, cache invalidation and access checks.
5. **Authorization.** Verify `FileRead`/`FileWrite` and path-scoped grants through real syscalls, including denied writes, denied create, denied unlink and sibling-prefix traversal paths.
6. **Capacity and failure injection.** Exercise full disk, maximum filename, maximum file length, out-of-range offset, device I/O errors and failed VFS publication. Ensure reported errors and persisted data agree.

## Acceptance gate

- Unit and integration tests for each risk above, including injected failures.
- QEMU 1/2/4 CPU test matrix; tests must inspect *persisted data after remount*, not only pass markers.
- `cargo clippy` warning-denying gate for kernel and relevant targets, plus host integration tests.
- Real hardware disk tests (or explicitly documented waiver) for persistence and power-loss.
- Record exact commands, SHA, logs, failures and outcomes in the Architecture & Codebase Guide.
- Keep this PR in draft until the acceptance evidence exists; do not claim production readiness on merge alone.

## Next implementation priority

Make native durable commit and VFS publication linearizable with respect to concurrent writers, without holding the VFS registry lock during disk I/O. Add an explicit per-file operation coordination strategy and a regression test that demonstrates the race before changing behavior.
