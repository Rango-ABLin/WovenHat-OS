# Persistent session account import audit — 2026-10-20

## Scope

The hardware-demo boot path now imports `/mnt/etc/wovenhat/accounts` from the
mounted FAT32 volume into the read-only `/etc/wovenhat/accounts` session path.
The source is bounded to one VFS node, must contain at least one syntactically
valid `name:salt-hex:hash-hex` line, and is rejected if it is missing required
fields, oversized, or not a regular file. PINs are never stored.

When the persistent source is absent, the existing disposable QEMU accounts
remain available for acceptance (`ada/1234`, `lin/9876`). A present but bad
source fails closed before the desktop session starts.

## Verification

- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features hardware-demo -- -D warnings` — passed with packaged app images and the test trust key.
- `cargo test --all` — passed.
- Stage 20.12 sign-in QEMU acceptance — passed on 1 CPU; evidence in `target/woven-shell-sign-in-1cpu`.
- Stage 20.12 sign-in QEMU acceptance — passed on 2 CPUs; evidence in `target/woven-shell-sign-in-2cpu`.
- Stage 20.12 sign-in QEMU acceptance — passed on 4 CPUs; evidence in `target/woven-shell-sign-in-4cpu`.

## Limits

This slice does not implement account creation, password change or rotation,
credential-store integrity protection, or a memory-hard password KDF. The
persistent fixture was not installed into the shared developer disk, so that
disk remains unchanged for interactive testing.
