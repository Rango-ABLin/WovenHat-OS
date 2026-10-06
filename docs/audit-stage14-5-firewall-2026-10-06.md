# Stage 14.5 WovenGuard firewall policy audit

This candidate adds a bounded IPv4 packet-policy evaluator above the accepted
Stage 14.4 route-table foundation. It has sixteen fixed rule slots, explicit
ingress/egress and protocol selectors, address/port wildcards, generation-safe
removal, and an implicit deny when no rule matches.

The self-test proves an allowed UDP DNS-shaped egress packet, rejects the same
selector on ingress, revokes the rule, and rejects a second stale removal.
No packet slice is retained, no capability is broadened, and the evaluator is
not yet connected to live socket admission.

Required acceptance: Stage 14.5 1/2/4 CPU QEMU gate, host regressions,
warning-denying kernel Clippy, and all prior network gates. Physical firewall,
connection tracking, NAT, IPv6 policy, and userspace policy management remain
out of scope.

## Acceptance results

- `cargo check --manifest-path kernel/Cargo.toml --features stage14-5-test` — passed.
- `cargo clippy -p wovenhat-kernel --features stage14-5-test --target x86_64-unknown-none -- -D warnings` — passed.
- `python -m py_compile scripts/test-network-qemu.py` — passed.
- Stage 14.5 live QEMU network gate — passed on 1, 2, and 4 CPUs.
- Preserved serial evidence:
  - `target/network-regression-1-debug/serial.log`
  - `target/network-regression-2-debug/serial.log`
  - `target/network-regression-4-debug/serial.log`

The repository-wide `cargo test --workspace` command remains unsuitable for
this freestanding `no_std` kernel: its existing test build attempts to link
the kernel's allocator and panic handler with `std` and fails before running
tests. This is recorded as a harness limitation, not treated as stage
acceptance evidence.
