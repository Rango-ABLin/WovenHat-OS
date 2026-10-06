# Stage 14.2F Neighbor cache and DAD state audit — 2026-10-06

This increment adds an eight-entry expiring neighbor cache, checksum-validated
Neighbor Advertisement application, and explicit Duplicate Address Detection
state transitions. Invalid message types, checksums, option shapes, and table
capacity fail closed. State is bounded and does not retain packet slices.

The implementation does not yet transmit Neighbor Solicitations, integrate a
live IPv6 interface, implement DHCPv6, or qualify physical IPv6 hardware.

Verification requires feature build and Clippy, the complete release matrix,
and Stage 14.2 focused QEMU acceptance on 1/2/4 CPUs with markers A–F,
including `[S14.2F] WovenNet neighbor state and DAD: PASSED`.
