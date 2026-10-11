# Stage 14.5D WovenGuard parser/transport hardening audit

## Scope

The `stage14-5d-test` firewall candidate now classifies IPv4/IPv6 through up to
two VLAN tags, rejects truncated Ethernet/VLAN frames and excessive tag
nesting, rejects fragmented IPv4 and non-atomic IPv6 fragments, and validates
TCP data offsets, UDP lengths, and minimum ICMP/ICMPv6 sizes before extracting
policy metadata. Other structurally valid IP protocols are represented as
portless `Other` traffic, allowing explicit protocol-specific rules and the
`Any` wildcard to work without conflating a valid unsupported protocol with
malformed input. Non-IP Layer-2 traffic remains preserved. Existing Stage 14.5B
synthetic UDP fixtures now include consistent IPv4/IPv6 and UDP lengths.

The feature self-test covers one/two VLAN tags and rejects deeper/truncated
tags; rejects IPv4 and IPv6 fragments; checks TCP data offsets, UDP lengths and
ICMP message minima; walks multiple IPv6 extension headers; and verifies
protocol-specific and wildcard matching for GRE.

## Validation performed

- `cargo build --features stage14-5d-test` — passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage14-5d-test -- -D warnings` — passed.
- `python3 scripts/test-release.py` — host kernel/host Clippy and all 20 host integration targets (79 tests) passed. The script stopped at `memory-1-debug` before starting QEMU.
- `cargo test --workspace --no-fail-fast` — not a supported kernel test invocation; it attempted to build the `no_std` kernel binary as a Rust test target and failed with missing `std`/duplicate panic-handler diagnostics.

## Acceptance status and limits

Stage 14.5D is **not accepted**. The release harness could not run its first
QEMU boot because this environment has neither `qemu-system-x86_64` nor OVMF.
Consequently, the required 1/2/4-CPU Stage 14.5D network gate and GitHub
acceptance remain outstanding.

The parser and Wi-Fi enforcement paths are compiled behind the
`stage14-5d-test` feature and do not establish default-build production firewall
coverage. The parser does not verify IP or transport checksums, IPv6 jumbograms
are unsupported, and no routed Forward datapath or physical NIC qualification
is claimed. Non-atomic IP fragmentation is deliberately rejected.

## Next acceptance prerequisites

Run the dedicated Stage 14.5D workflow on the exact source tip, including
warning-denying build/Clippy and the inherited live networking regression gate
on 1, 2, and 4 CPUs. Review the generated serial and QEMU logs before recording
acceptance or proceeding to stateful connection tracking.
