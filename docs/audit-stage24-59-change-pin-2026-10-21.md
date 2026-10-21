# Stage 24.59 Change-PIN acceptance audit — 2026-10-21

## Delivered

- Added a deterministic kernel/QEMU self-test for the real WovenShell Change-
  PIN UI path.
- Covered sign-in, Quick Settings, current/new/confirmation PIN entry,
  durable record rotation, relock, old-PIN rejection, and new-PIN unlock.
- Synchronised the kernel identity authority and `/etc/wovenhat/accounts` with
  one bounded kernel-generated salt. The existing rollback path restores the
  authority if the account-record rewrite fails.

## Verification

- Production-feature kernel Clippy with packaged signed system-app images:
  passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test -p libwoven --quiet`: 95 passed.
- Stage 24.59 QEMU acceptance on 1, 2, and 4 CPUs: passed.

The serial and QEMU logs are preserved in
`audit-artifacts/stage24.59-change-pin-20261021`.

## Limits and next prerequisites

This stage closes the password-change gap only. Account creation remains
unimplemented. Physical TPM activation, real-device reboot recovery, and
hardware/USB qualification remain open release gates; this work performed no
USB writes.
