# Phase C — Native hardware reconciliation (2026-11-23)

**Author**: systems/agent pass on branch `phase-b/bottom-taskbar-core`, base `07f4c8f2`
**Decision**: `PHASE C ACCEPTED — BOUNDED SOFTWARE/QEMU ONLY`
**Native qualification**: still `BLOCKED`, unchanged, and not relabelled.

This report reconciles what "native hardware support" currently means in this
repository, what Phase C actually delivered (a native Intel e1000/e1000e NIC
driver integrated with the existing smoltcp stack), and what remains genuinely
open. It deliberately follows the existing records rather than replacing them:

- `docs/hardware-qualification-stage29-2026-11-20.md` — the three-zone lab model
  (Z1 read-only, Z2 emulated/software, Z3 native destructive) and its results.
- `docs/release-qualification/EXTERNAL-REVIEW-AND-HARDWARE.md` — "BLOCKED — no
  complete native lab matrix is authorized or present."
- `docs/release-qualification/hardware-support-record.json` — all 17 categories
  remain `blocked` with `environment: native`.

Nothing in Phase C touched a physical device, disk, firmware setting or boot
entry. No Secure Boot change, no raw write, no power-loss cycle, no USB target.

## 1. Gap matrix at the start of Phase C

The earlier audit found the driver framework (`kernel/src/driver.rs`), PCI/PCIe
enumeration (`kernel/src/hal/pci/*`), AHCI/SATA, NVMe, xHCI/USB + HID, ACPI,
power and SMP already implemented and QEMU-accepted. Two genuine gaps remained:

| Domain | Status before Phase C | Native evidence |
|---|---|---|
| Network (Ethernet) | virtio-net only — a paravirtual transport, not a real device | none (Z3 blocked) |
| GPU / display | virtio-gpu + framebuffer; native GPU deferred by the roadmap | none (Z3 blocked) |
| Storage, USB/HID, audio, ACPI, power, SMP | implemented and QEMU-accepted | none (Z3 blocked) |

The network gap mattered because every prior network acceptance marker was
produced over virtio-net. A native driver is a precondition for ever qualifying
the `ethernet` category; it is not that qualification.

## 2. What Phase C delivered

### 2.1 Native Intel e1000/e1000e transport

`kernel/src/e1000.rs` (new) implements a polling-first legacy-descriptor driver
for Intel `8086` functions:

| Device | PCI ID | QEMU model |
|---|---|---|
| 82540EM | `0x100E` | `-device e1000` |
| 82542 | `0x1000` | — |
| 82543GC | `0x1004` | `-device e1000-82544gc` family |
| 82545EM | `0x100F` | `-device e1000-82545em` |
| 82574L | `0x10D3` | `-device e1000e` |

Only these five IDs are admitted; the driver does not bind by class code alone,
so unsupported Intel NICs stay unbound rather than being driven incorrectly.

Design mirrors `kernel/src/virtio_net.rs` deliberately:

- One MMIO window: BAR0 pages `0x0000`–`0x5FFF` are mapped once during init, so
  the dataplane never re-enters the paging lock (rank 10) under the transport
  lock (rank 30). This was an actual lock-order inversion found and fixed during
  the work.
- One bounded DMA reservation: 9 contiguous frames (33 024 B) holding an 8-entry
  RX ring, an 8-entry TX ring, and eight 2048-byte buffers per direction. Every
  region is 16-byte aligned so ring bases are legal.
- Polling only: interrupts are masked after reset (`IMC = 0xFFFF_FFFF`) and
  completion is reaped from descriptor `DD` bits. MSI/MSI-X remain follow-on
  work.
- Single outstanding TX descriptor (`tx_outstanding`), so `Device::transmit`
  only hands smoltcp a token that can really submit.
- Ring indices stay inside the ring: `RDT` and `TDT` are written as modulo ring
  indices, not absolute frame counters. Writing unbounded values was found to
  let the device replay stale descriptors once the ring wrapped.

### 2.2 Integration into the existing network stack

- `kernel/src/network.rs`: new `NicKind { Virtio, E1000 }`; `VirtioSmolDevice`
  became `SmolDevice` carrying its kind; RX/TX dispatch, `transport_mac()` and
  `transport_name()` follow the same ordering as `init()`.
- `network::init()` prefers virtio-net and falls back to e1000; when neither is
  present it returns `InitError::NoTransport` and the boot continues offline.
  Existing virtio behaviour is unchanged.
- `kernel/src/main.rs`: `[NETTEST] <transport> + smoltcp initialized` now names
  the real transport instead of hardcoding virtio-net.
- `kernel/src/shell.rs`: `cmd_net` reports both probe results.
- `kernel/src/hal/pci.rs`: `enable_memory_bus_master` is no longer feature-gated,
  because the always-compiled e1000 module needs it under bare `qemu-test`.

### 2.3 One behavioural correction, not an assertion weakening

`qemu_runtime_self_test()` drives DHCP, DNS, ICMP and **host-forwarded**
UDP/7000 + TCP/8080 traffic. It now only runs in the dedicated `network-test`
image. Reason: the Stage 13.2 PCIe-topology gate attaches an unforwarded e1000
(`-netdev user`, no `hostfwd`). Before Phase C that device was unbound, so the
live test never ran there; once e1000 binds, the same gate tried to run a
host-dependent round trip with no host peer and halted on `UDP: TIMEOUT`. The
correct scoping is that a harness proving PCIe topology does not also have to
supply slirp forwarding, so the live regression belongs to the image whose
harness does. Every marker any gate actually requires is unchanged:

- `test-network-qemu.py` still requires `[NETTEST] DHCP/DNS/ICMP/UDP/TCP: PASSED`.
- Stage 14.x still runs the live test (all Stage 14 features chain `network-test`).
- No stage marker was removed, relaxed, retimed or made conditional on success.

## 3. Acceptance evidence (software/QEMU only)

Environment: QEMU tcg, `q35`, 256M, OVMF, project-local build dirs. All runs
below completed with exit 33 and every required marker.

| Gate | Feature / NIC | CPUs | Result |
|---|---|---|---|
| `scripts/test-network-qemu.py --nic e1000` | `network-test` | 1 | PASS |
| `scripts/test-network-qemu.py --nic e1000` | `network-test` | 2 | PASS |
| `scripts/test-network-qemu.py --nic e1000` | `network-test` | 4 | PASS |
| `scripts/test-network-qemu.py --nic e1000e` | `network-test` | 1 | PASS |
| `scripts/test-network-qemu.py --nic e1000e` | `network-test` | 2 | PASS |
| `scripts/test-network-qemu.py --nic e1000e` | `network-test` | 4 | PASS |
| `scripts/test-network-qemu.py --nic e1000` | `stage14-5d-test` (IPv6 + firewall chain) | 1 | PASS |
| `scripts/test-network-qemu.py --nic virtio` | `network-test` (regression) | 1 | PASS |
| `scripts/test-stage10-runtime.py --stage 13.2` | `stage13-2-test`, e1000 attached | 1, 2, 4 | PASS |

Each network run includes: DHCP lease, DNS resolution, ICMP reachability, a
host-verified UDP echo on 7000 and a host-verified bidirectional TCP round trip
on 8080. The Stage 14.5 run additionally required the whole Stage 14.1–14.5D
chain, including `[S14.5D] firewall parser/transport hardening PASSED`.

Static and host gates, all `-D warnings` (no warnings suppressed):

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none` for default,
  `qemu-test`, `network-test`, `stage13-2-test`, `stage14-1-test`,
  `stage14-5d-test`.
- `cargo clippy --all-targets -- -D warnings`.
- `cargo test --workspace` (256 libwoven lib tests, 1 pre-existing ignored).
- `cargo fmt --all -- --check`.

Evidence: `audit-artifacts/phase-c-e1000-network-20261123/` (per-CPU serial
logs for e1000, e1000e, the Stage 14.5 e1000 run and the virtio regression) and
`audit-artifacts/stage13.2-{1,2,4}cpu-*/`.

## 4. What this does and does not establish

**Established (QEMU/software):** WovenHat now has a native, non-paravirtual
Ethernet driver that carries the full IPv4 production networking suite through
real host-forwarded traffic on both admitted device models, at 1/2/4 CPUs, over
the same smoltcp runtime used by normal boots.

**Not established:**

- No native (Z3) qualification of any kind. The `ethernet` category in
  `hardware-support-record.json` stays `blocked`; Tier 1 still requires a
  `passed` observation with `environment == "native"`.
- No claim about real 82540EM/82574L silicon: PCI configuration-space
  behaviour, PHY/link negotiation, EEPROM MAC handling, bus-mastering with real
  IOMMU/cache-coherency behaviour, interrupt delivery and error recovery on
  real hardware are all untested. The driver forces full-duplex link-up with no
  PHY negotiation, which is correct for the QEMU models and is *not* evidence
  about a physical NIC.
- No MSI/MSI-X, no interrupt-driven dataplane, no multi-descriptor TX pipelining,
  no checksum/TSO offload, no jumbo frames, no link-state change handling.
- Native GPU remains open and deliberately deferred by the roadmap.

## 5. Remaining work, in dependency order

1. **Physical Ethernet qualification (Z3, blocked).** Requires, in order: an
   owner-selected spare non-boot device, Administrator elevation, and a signed
   authorization covering the erase/power-cycle risk. Then bind the driver to a
   real e1000/e1000e function, capture link negotiation, MAC/EEPROM handling and
   throughput, and write `wovenhat.hardware-qualification/v1` observations.
2. **Interrupt-driven TX and MSI/MSI-X**, replacing the poll-only dataplane, with
   the same bounded single-outstanding-TX contract.
3. **Real link-state handling** (PHY negotiation, link down/up recovery) instead
   of forced full-duplex link-up.
4. **Throughput work**: multiple outstanding TX descriptors, ring size tuning,
   and descriptor batching.
5. **Native GPU**, still deferred per `docs/master-development-roadmap.md`
   ("Do not make NVIDIA/AMD/Intel native GPU support a prerequisite for the first
   GUI release").
6. **Independent review and production signing custody**, both still blocked per
   `docs/release-qualification/EXTERNAL-REVIEW-AND-HARDWARE.md`.
