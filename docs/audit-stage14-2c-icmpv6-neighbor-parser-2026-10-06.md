# Stage 14.2C ICMPv6 Neighbor Discovery parser audit — 2026-10-06

## Scope

This increment adds a bounded parser for ICMPv6 Router Solicitation,
Router Advertisement, Neighbor Solicitation, and Neighbor Advertisement
envelopes. It validates type/code, fixed-body length, non-multicast neighbor
targets, and complete non-zero-length options, with a 1 KiB input bound. The
parser borrows only the caller-owned option slice and does not retain packet
state for asynchronous use.

The parser deliberately does not verify the IPv6 pseudo-header checksum or
perform stateful Neighbor Discovery actions. Duplicate Address Detection,
router lifetime/prefix processing, DHCPv6, live IPv6 sockets, and physical
qualification remain outside this increment.

## Verification

- `cargo build --features stage14-2-test`
- warning-denying feature Clippy
- `scripts/test-network-qemu.py --feature stage14-2-test` on 1/2/4 CPUs
- complete release validation

Each network serial log must contain `[S14.2A]`, `[S14.2B]`, and `[S14.2C]`
success markers.
