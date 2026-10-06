# Stage 14.2J IPv6 interface ingress audit — 2026-10-06

This increment adds a bounded ingress coordinator for caller-owned ICMPv6
packets. It verifies the IPv6 pseudo-header checksum, parses the envelope,
dispatches Router Advertisement and Neighbor messages into existing bounded
state, and emits explicit ingress events. Tampered packets are rejected before
state mutation; packet slices are not retained.

This remains a software interface boundary. IPv6 address configuration,
controlled packet egress, live sockets, and physical NIC/radio qualification
remain open. DHCPv6 protocol framing is covered by the preceding Stage 14.2H
foundation but live lease exchange is not.

Verification requires feature build and warning-denying Clippy, the complete
release matrix, and focused Stage 14.2 QEMU acceptance on 1/2/4 CPUs with
markers A–J.

Verification on the exact source tip:

- `cargo build --features stage14-2-test` — PASS.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage14-2-test -- -D warnings` — PASS.
- The focused QEMU gate passed on 1, 2, and 4 CPUs.
