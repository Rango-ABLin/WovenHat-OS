# WovenInit boot-path repair and read-only image smoke — 2026-11-27

## Scope

While diagnosing the prepared `hardware-demo` image, the strict shell gate
reported `WovenInit essential readiness barrier was not reached`. The serial
evidence showed the desktop installed all 11 signed applications and reached
`[BOOT] desktop-first runtime ready`, but never emitted the four-node WovenInit
readiness sequence or `[WOVENINIT] essential services ready mask=0xf`.

This was a real boot-path regression, not an obsolete harness expectation:
normal boot had stopped calling `woven_init::start()` and instead used an older
inline service-launch path. The existing architecture and acceptance documents
require the coordinator to own production-service dependency order, liveness
readiness and rollback. The repair restores that contract rather than changing
the marker or the gate.

## Change

- `kernel/src/main.rs`: restored
  `#[cfg(feature = "system-apps")] mod woven_init;`.
- `kernel/src/main.rs`: restored
  `start_production_system_services()` as a delegate to `woven_init::start()`.

No assertion, timeout, capability boundary, identity policy, shell source or
desktop qualification harness was weakened. Concurrent uncommitted P0 desktop
diagnosis work and other owners' files were left untouched and were not staged.

## Validation

All records are under `audit-artifacts/z3-woven-init-repair-20261127/`; the
machine-readable index is `validation-summary.json`.

### Host gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| Warning-denying kernel Clippy for `system-apps`, `hardware-demo`, `woven-init-failure-test`, `stage31-service-supervisor-test` | 4/4 PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo test --workspace` | PASS |
| `cargo test -p libwoven --all-features --locked` | PASS |

### QEMU acceptance gates

| Gate | CPUs | Result | Evidence |
|---|---:|---|---|
| WovenInit readiness-failure rollback | 1, 2, 4 | PASS; `[PHASE C] WovenInit readiness failure rollback: PASSED` | `woven-init-failure-{1,2,4}cpu.log`, `woven-init-failure-{1,2,4}cpu-serial.log` |
| Signed-service crash/restart | 1, 2, 4 | PASS; update and search recovery markers | `supervisor-{1,2,4}cpu.log`, `supervisor-{1,2,4}cpu-serial.log` |
| Rebuilt-image read-only smoke | 1, 2, 4 | PASS; all six strict markers, 1280×800 framebuffer, QEMU exit 0 | `z3-qemu-image-smoke-woveninit-20261127/*cpu-17957926*/result.json` |

The rebuilt disposable image is
`audit-artifacts/native-hardware-image-woveninit-20261127/wovenhat-hardware-demo.img`,
35,717,120 bytes, SHA-256
`cc943465112c2f5488bb90a4ae31cad3e38d708ecd266aacfeed67c9ef88da22`. It was
built from source commit `3e929570` plus the working-tree repair; the exact
worktree list, including concurrent owner edits, is in that directory's
`image.json`. The USB prepared earlier was not rewritten.

The first version of the smoke harness falsely treated the APPS marker as fatal
because a substring check for `stalled` matched `installed`. Those three failed
runs are preserved under
`audit-artifacts/z3-qemu-image-smoke-woveninit-20261127/*cpu-1795792[45]*`. The
harness was corrected with word boundaries; no boot marker, fatal condition,
timeout or screenshot check was removed or relaxed. The corrected harness is
preserved as `qemu_image_smoke_v2.py`.

A post-repair 1-CPU hardware-demo shell gate reached
`[WOVENINIT] essential services ready mask=0xf`, then failed only at the
concurrent owner's already-open P0 sign-in timing gap. That gate is therefore
recorded as partial, not passed. Its transcript is
`audit-artifacts/z3-shell-gate-check-20261127/test-shell-fixed-1cpu.log` and the
serial copy is `hardware-demo-marker-1cpu-serial.log`.

## Limits

- This is QEMU and read-only image evidence. It cannot qualify native hardware
  boot, firmware, TPM, USB device functions or a production release.
- Stage 29 native boot phases remain blocked pending owner-controlled reboot,
  physical console/serial capture and power control.
- The full desktop sign-in timing gate remains unresolved by its owner and is
  not claimed passed here.
- No hardware report classification was changed: Tier 2 remains valid with zero
  native-passed categories.
