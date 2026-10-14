# Stage 14.5D network revalidation — 2026-10-14

The current tree passed the live Stage 14.5D network suite on 1, 2, and 4
CPUs. Each run completed DHCP, DNS, ICMP, host-verified UDP and TCP
round-trips, SMP/TLB checks, and the firewall transport/path marker with exit
33. The harness also verifies the host-forwarded UDP and TCP payloads rather
than accepting only an in-guest marker.

Evidence is retained in the harness output directories:

- `target/network-regression-1-debug/serial.log`
- `target/network-regression-2-debug/serial.log`
- `target/network-regression-4-debug/serial.log`

This is a QEMU virtio-network qualification. Native NIC/radio drivers,
physical link behavior, and hardware-specific offload/interrupt behavior
remain outside this evidence.
