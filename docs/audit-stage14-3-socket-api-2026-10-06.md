# Stage 14.3 WovenNet socket API audit — 2026-10-06

Stage 14.3 hardens the existing socket ABI boundary without changing the
accepted descriptor, owner, generation, or asynchronous completion model.
Socket connect endpoints now fail closed for zero ports, unspecified IPv4
addresses, and IPv4 multicast destinations. Packed IPv4 endpoint decoding is
validated before it reaches smoltcp, and valid endpoint packing round-trips
are covered by a deterministic self-test.

This is bounded software validation. The current transport is IPv4-only;
live IPv6 sockets, routing-table policy, and physical NIC qualification remain
outside this stage.

Verification on the exact source tip:

- `cargo build --features stage14-3-test` — PASS.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage14-3-test -- -D warnings` — PASS.
- `scripts/test-network-qemu.py --feature stage14-3-test --cpus 1` — PASS.
- The same QEMU gate on 2 and 4 CPUs — PASS.
