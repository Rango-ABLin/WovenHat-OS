# Stage 31 network-loss slice — 2026-10-22

`scripts/test-network-qemu.py` now supports `--link-loss`. The harness starts
QEMU with an opt-in QMP control channel, waits for the normal DHCP/DNS/ICMP and
UDP proof, sends `set_link` down for `net0`, sends `set_link` up, and then
requires the host TCP echo, the guest network markers, the SMP/TLB marker, and
exit 33. QMP errors and all existing guest failure markers are fatal.

Passing commands:

```text
python scripts/test-network-qemu.py --cpus 1 --timeout 300 --link-loss
python scripts/test-network-qemu.py --cpus 4 --timeout 300 --link-loss
```

Evidence is retained in:

- `target/network-regression-1-debug/serial.log`
- `target/network-regression-4-debug/serial.log`

This slice proves recovery from an emulated virtio link flap. It does not
qualify physical NIC loss, packet corruption, Wi-Fi recovery, driver restart,
or the complete Stage 31 torture suite.
