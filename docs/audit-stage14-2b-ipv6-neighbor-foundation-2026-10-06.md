# Stage 14.2B IPv6 neighbor foundation audit — 2026-10-06

## Scope

This increment adds production-reusable, allocation-free IPv6 address and
prefix primitives: prefix containment, link-local/multicast classification,
and RFC 4291 solicited-node multicast derivation. The acceptance self-test
exercises the exact primitives used by the future Neighbor Discovery path and
fails closed on malformed prefix lengths.

This is not yet IPv6 packet transport, ICMPv6 Neighbor Solicitation/Advertisement
handling, Duplicate Address Detection, Router Solicitation/Advertisement,
DHCPv6, or physical IPv6 qualification.

## Verification

The following checks are required before this commit is accepted:

- `cargo build`
- warning-denying freestanding kernel Clippy
- host tests
- `cargo build --features stage14-2-test`
- warning-denying feature Clippy
- `scripts/test-network-qemu.py --feature stage14-2-test` on 1, 2, and 4 CPUs
- complete release validation

The resulting serial evidence must contain both `[S14.2A]` and `[S14.2B]`
success markers for each CPU configuration.

## Limits and next prerequisite

No physical NIC or IPv6 wire claim is made. The next prerequisite is a bounded
ICMPv6 Neighbor Discovery message/parser boundary backed by owned packet data,
before enabling live IPv6 interface configuration.
