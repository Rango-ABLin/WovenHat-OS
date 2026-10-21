# Production account provisioning audit — 2026-10-21

## Scope

Commit `7bbcb0f8` moves bounded persistent-account import into the normal
`system-apps` boot path. After `/mnt` is mounted, a release boot imports a
valid `/mnt/etc/wovenhat/accounts` file into `/etc/wovenhat/accounts` and
halts on malformed, oversized, non-file, or unreadable input. If no source
file exists, boot continues for first-boot account setup. The disposable
`hardware-demo` fallback preserves an already imported account file.

## Verification state

- Source review confirmed that storage mounting precedes account import and
  that `hardware-demo` does not overwrite an imported account file.
- `git diff --check`: passed after the commit.
- A bare production kernel Clippy invocation was rejected by the compile-time
  image contract because the signed application image variables were absent;
  this is expected and is not a valid release configuration.
- The harness-backed `system-apps` 1-CPU gate packaged all ten signed apps,
  then was stopped while the nested bootloader build waited on a Cargo build
  lock held by concurrent shared-agent work. No QEMU result was claimed.

## Open acceptance work

Run the harness-backed production Clippy and `scripts/test-shell-qemu.py
--system-apps` on 1/2/4 CPUs after the shared Cargo lock clears. Re-run the
global formatting, all-targets Clippy, host tests, and the relevant prior
stage gates. Physical USB boot and TPM qualification remain separate release
gates and were not performed here.
