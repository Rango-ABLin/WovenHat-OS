# CI repair audit - 2026-11-27

## Scope

This pass repaired current `master` CI defects without weakening an assertion,
timeout, offline/locked requirement, warning gate or QEMU marker contract.

Changed contracts:

- Restored `#[cfg]`-gated declarations for `wovenweb_cxx_gate`,
  `wovenweb_memory_gate` and `wovenweb_net_gate` in `kernel/src/main.rs`.
  Their self-test call sites already existed; the modules were simply not
  declared, so every WovenWeb native feature failed to compile.
- Replaced the stale square-grid wallpaper oracle in `stage20_18_self_test`
  with the accepted diagonal weave and named local constants. The old oracle
  contradicted the rest of the Stage 20 framebuffer checks.
- Updated host Python tests to the current source locations and APIs:
  UTF-8 explicit reads, `tokens.rs` constants, `firmware_drives()` /
  `copy_firmware_vars()`, case-insensitive Windows temp-path comparison, and
  removal of a duplicate test method that overrode the correct APPS-aware
  assertion.
- Added a locked dependency fetch step to the Windows acceptance workflow.
  The authoritative gate remains locked and offline; the fetch only populates
  the runner cache before the clean-room provenance and SDK signer checks run.

Concurrent P0 owner files were not staged or modified by this repair. No QEMU
acceptance chain was started, no USB device was touched, and no hardware or
native validation claim is made here.

## Local evidence

Commands were run with the project-local Cargo directories used by the Stage
10.7 launcher.

| Gate | Result |
| --- | --- |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features wovenweb-memory-test --locked -- -D warnings` | 0 |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features wovenweb-net-test --locked -- -D warnings` | 0 |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features wovenweb-cxx-test --locked -- -D warnings` | 0 |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage20-18-test --locked -- -D warnings` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 |
| `cargo fmt --all -- --check` | 0 |
| `cargo test --workspace --locked` | 0 |
| `python -X utf8=0 -m unittest discover -s tests -p 'test_*.py'` | 288 tests, OK |
| `git diff --check` | 0 |
| Workflow YAML parse | 9 acceptance steps parsed |

The WovenWeb kernel feature lints needed their fixture image environment
variables because `include_bytes!(env!(...))` is evaluated during compilation.
Dummy fixture files were used only for compile/lint validation; no runtime
fixture or acceptance result was inferred from them.

## CI status and limits

At the last check, the earlier commit `7634d8da` still had kernel run
`46024406213` in progress and Windows QEMU run `46024406110` queued; WovenWeb
run `46024406139` had failed on the missing module declarations fixed here.
Those runs predate this repair. A new push is required to validate the new
commit. Full CI green is not claimed until those replacement runs complete.

Stage 29 native hardware qualification remains blocked on the owner-controlled
reboot, physical console/serial capture and power control. This CI repair does
not advance Tier 2 or any native qualification category.
