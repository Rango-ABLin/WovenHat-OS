# Stage 31 packet-corruption boundary audit — 2026-10-22

Added `scripts/test-stage31-packet-corruption.py`, which runs the real network
QEMU harness with the Stage 14.2 ICMPv6 checksum and ingress-hardening gates.
Malformed checksum input must be rejected without poisoning the network state.

Verification passed on 1/2/4 CPUs. Each run also completed DHCP, DNS, ICMP,
host UDP/TCP round trips, SMP/TLB markers, and exit 33. The decisive guest
marker is `[S14.2E] WovenNet ICMPv6 checksum boundary: PASSED`.

Evidence is retained under
`audit-artifacts/stage31-packet-corruption-{1,2,4}cpu`.

This is partial Stage 31 coverage: deterministic malformed-checksum rejection
is proven, but randomized corruption across the complete Ethernet/IP/TCP/UDP
receive paths is still open.
