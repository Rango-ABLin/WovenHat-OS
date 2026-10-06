# Stage 14.4 WovenNet routing-table audit — 2026-10-06

Stage 14.4 adds a bounded routing-table foundation with eight route slots.
Routes carry a generation handle, prefix length, gateway, and metric. Lookup
selects the longest matching prefix, then the lowest metric, then the oldest
generation. Invalid prefix lengths and zero gateways fail closed; removal
requires the exact generation-tagged route value.

This is deterministic software validation. It does not yet mutate the live
smoltcp route set, provide policy/routing daemons, or qualify physical NICs.

Verification requires the Stage 14.3 regression chain, build, warning-denying
kernel Clippy, and the focused QEMU network gate on 1/2/4 CPUs.
