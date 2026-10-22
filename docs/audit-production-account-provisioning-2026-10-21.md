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
- The root release-image build initially reported the legacy `change_pin`
  fallback as dead code under `-D warnings`; the shared agent cfg-gated that
  fallback to non-`system-apps` builds.
- Harness-backed signed `system-apps` shell acceptance passed on 1, 2, and 4
  CPUs. Each run packaged all ten signed applications and verified normal
  release shell boot, PS/2/IOAPIC input, and the SMP smoke path. Evidence is
  in `target/shell-regression-{1,2,4}cpu` and the captured retry logs.
- Signed-image production kernel Clippy with `-D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all`: passed.
- `git diff --check`: passed.

## Open acceptance work

The repository-wide `cargo fmt --all -- --check` still reports pre-existing
formatting diffs outside this slice and must be resolved or explicitly
qualified before a release claim. Relevant prior-stage gates still need to be
rerun as part of the complete release chain. Physical USB boot and TPM
qualification remain separate release gates and were not performed here.
