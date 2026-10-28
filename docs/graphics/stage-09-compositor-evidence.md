# Graphics roadmap Stage 09 — compositor and composition scheduler

Date: 2026-10-25  
Gate: `stage09-compositor-test`  
Scope: damage scheduling, stacking/focus, input routing, and teardown

## Implementation

Stage 09 exposes a roadmap-native kernel/QEMU gate while reusing the accepted
surface and userspace compositor contracts:

- damage-only composition copies only the client-declared region and leaves
  undeclared pixels unchanged;
- the compositor sleeps until damage, owns the display lease, and releases it
  on exit;
- overlapping windows are stacked and raised through focus changes;
- keyboard input is delivered to the focused client exactly once;
- private/foreign surfaces and unauthorized damage operations are refused;
- client, compositor, focus, and surface ownership are released on teardown;
- the damage fixture is removed before the window-manager fixture is installed,
  preventing stale VFS state from masking the second test.

Resize, tiling, and killed-window display recovery are covered by the existing
accepted Stage 20.8 gate (`scripts/test-woven-shell-qemu.py --stage 20.8`),
which is the complementary desktop recovery qualification for this scheduler
stage.

## Acceptance evidence

The following commands passed with QEMU exit code 33 and the required marker:

```text
python scripts/test-hello-gui-compositor-qemu.py --stage 09 --cpus 1
python scripts/test-hello-gui-compositor-qemu.py --stage 09 --cpus 2
python scripts/test-hello-gui-compositor-qemu.py --stage 09 --cpus 4
```

Each serial log contains the Stage 19.9 damage assertions, the Stage 19.11
stacking/focus and key-routing assertions, and:

```text
[S09] compositor scheduler: PASSED
```

The freestanding warning-denying lint gate also passed:

```text
cargo clippy --manifest-path kernel/Cargo.toml \
  --target x86_64-unknown-none \
  --features stage09-compositor-test -- -D warnings
```

The host `--all-targets` lint remains a documented tooling limitation: host
test compilation links the freestanding kernel with `std` through host-only
WebP dependencies, producing duplicate `panic_impl` and allocator handlers;
the guest target gate remains clean and no test assertions or timeouts were
relaxed.
