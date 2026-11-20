# M5 Baseline Audit

Date: 2026-11-20
Branch: `master` (HEAD `6b54dce0`)
Scope: repository reconnaissance and build baseline only. No driver code was
changed by this audit.

## 0. Update (same day, after master moved)

- `master` advanced to `10740a4f` (two commits by the other owner). The M5
  branch `m5/hardware-qualification` was fast-forwarded to it. The working tree
  no longer has the uncommitted libwoven breakage described in section 1.
- On `10740a4f`, `cargo check -p wovenhat-kernel --target x86_64-unknown-none
  --features stage13-9-test` passes.
- Still open: warning-denying Clippy fails in `libwoven` (`too_many_arguments`,
  `render_outline`). This is a committed blocker.
- New finding: on untouched `master`, the 1-CPU QEMU `qemu-test` chain stops at
  `[FILE MMAP] regression tests: FAILED` (exit 35). The failing section is the
  large-asset window from commit `d7f42637`. Details: `M5_TEST_RESULTS.md`.
- The M5 roadmap entry is in `docs/master-development-roadmap.md`.

## 1. Repository state and blockers (as found at 6b54dce0)

- **Committed HEAD builds.** `git archive HEAD` extracted to a temp directory
  and `cargo check -p wovenhat-kernel --target x86_64-unknown-none --features
  stage13-12-test` completed with exit 0 (39 s, cached dependencies).
- **Working tree does not build.** `git status` shows two uncommitted modified
  files: `libwoven/src/lib.rs` (+89/-19) and `apps/woven-shell/src/main.rs`
  (+7). The same check on the working tree fails with 11 errors in `libwoven`
  (`E0422 cannot find struct OutlineOptions`). `OutlineOptions` is declared
  inside a `impl` block rather than at module scope. These edits belong to
  another owner per `docs/agent-coordination.md` (the libwoven glyph owner).
  They were left untouched. Any M5 kernel build is blocked until that owner
  lands or fixes them.
- **No M5 definition in the repository.** `docs/master-development-roadmap.md`
  and `docs/stage-status.md` are stage logs for TPM, desktop and graphics work.
  A search for "M5" found only GUI milestones (`docs/graphics/overview-desktop-*`,
  `docs/GNOME-Desktop-Claude-Prompt.md`). The M5 hardware-driver milestone
  text was supplied in the request and is not part of the roadmap that
  `AGENTS.md` makes authoritative. AGENTS.md requires stages to follow the
  roadmap in order.
- **Shared-tree coordination.** `docs/agent-coordination.md` lists active
  owners for `libwoven/src/lib.rs`, `libwoven/src/design_system.rs` and
  `apps/woven-shell/**`. M5 work that touches shared files must coordinate.
- **Not present:** WHQLab (no references anywhere in the repository). The
  Windows qualification tooling that does exist is `scripts/physical-host-preflight.ps1`,
  `scripts/physical-qualification-preflight.py`, `scripts/hardware-qualification-stimulator.py`,
  `scripts/run-hardware-qualification-matrix.py`, `scripts/qualify-hardware-report.py`,
  `scripts/inventory-target-disks.py` and `scripts/test-physical-tpm-readiness.ps1`.
- **Tooling present:** `cargo` via `C:\Users\banda\.cargo\bin`, and
  `C:\Program Files\qemu\qemu-system-x86_64.exe`. Kernel features use the
  `stageNN-test` naming, and kernel crate has `test = false` (host tests live in
  `tests/`).
- **Feature name correction.** The features are `stage13-12-test` style. There
  is no `stage10_7` feature; the Stage 10.7 gate is `stage10-7-test`.

## 2. Subsystem inventory

Status key: VERIFIED, IMPLEMENTED_UNVERIFIED, PARTIAL, MISSING, BLOCKED.
"Unverified" means the source exists and compiles for the committed HEAD in
the freestanding target, but no runtime or host test was executed in this audit.

| Subsystem | Source (kernel/src unless noted) | Current capability | Known deficiencies | Existing tests | Missing tests | Hardware available | Risk | Status |
|---|---|---|---|---|---|---|---|---|
| PCI config access, enumeration | `hal/pci.rs` (104 KB) | Config access, bus enumeration, ECAM via `hal/acpi.rs` MCFG | Not reviewed in depth in this pass | `tests/pci_topology.rs` | Multi-device and malformed-capability fixtures on hardware | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| PCI BAR sizing / decode | `hal/pci/bar.rs` | BAR decode and validation | Physical BAR-sizing safety with active devices not reviewed | `tests/pci_bar.rs` | Live-device BAR reprogram tests | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| PCI bridges / windows | `hal/pci/bridge.rs`, `bridge_transaction.rs` | Type-1 window encoding, transactional rollback | Bridge traversal on real multi-bridge topology untested | `tests/pci_bridge*.rs` | Nested-bridge QEMU topology | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| Resource allocation | `hal/pci/resource.rs`, `assignment.rs` | Bounded transactional allocator (`MAX_RANGES` 96) | No overlap test against real firmware-assigned ranges | `tests/pci_resource.rs`, `tests/pci_assignment.rs` | Firmware-range conflict fixture | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| MSI | `hal/pci/msi.rs` (13 KB) | MSI capability handling | Runtime delivery not verified in this pass | None found by name | Host capability-layout test | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| MSI-X | `hal/pci/msix.rs` (28 KB) | Capability decode and table/PBA layout validation, separated from MMIO | Table writes depend on driver BAR mapping | `tests/pci_msix_layout.rs` | Runtime vector delivery on QEMU | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| Interrupt vector routing | `hal/pci/routing.rs`, `vector.rs`, `driver.rs` (MSI/MSI-X activation) | Routing and vector allocation | Shared-interrupt and storm protection not verified | `tests/pci_routing.rs`, `tests/pci_vector.rs` | Storm and shared-line tests | QEMU only | High | IMPLEMENTED_UNVERIFIED |
| Driver binding / unbinding | `driver.rs`, `drivers.rs` | Bound lifecycle, `Unbinding` state, releases MSI/BAR leases | Teardown ordering under concurrent I/O not verified | None found | Bind/unbind stress | QEMU only | High | PARTIAL |
| IOMMU / VT-d | none (`grep iommu|vt-d|dmar` returns nothing in kernel/src) | None | No DMA isolation of any kind | None | n/a | n/a | Critical for DMA security | MISSING |
| Shared DMA API | per-driver: `nvme.rs` `DmaPage`, `ahci.rs` `DmaPage`, `hda.rs` `DmaPage`, `virtio_pci.rs` `DmaPages`, `virtio_net.rs` `DmaReservation`, `wifi_hw.rs` `OwnedDmaBuffer`/`DmaRing` | Each driver owns its own DMA page type | No shared allocation, mapping lifecycle or ownership API; duplicated logic | None shared | Shared-API tests | QEMU only | High | PARTIAL |
| NVMe | `nvme.rs` (18 KB), `block_io.rs` (`RootDevice::Nvme`) | Controller init via `init_at`, root device selection | Queue depth, PRP validation, timeout/reset recovery and flush semantics unverified; host tests absent | Stage 13.3 / 13.14 QEMU scripts (`scripts/test-stage13-14-storage.py`, `test-storage-qemu.py`) | Full M5 matrix (sections 5 of spec) | QEMU only | High | IMPLEMENTED_UNVERIFIED |
| AHCI / SATA | `ahci.rs` (19 KB), `block_io.rs` (`RootDevice::Ahci`) | Controller init, COMRESET-based port reset, `BlockDevice` impl | NCQ and hot-plug not implemented or tested; timeout recovery unverified; host tests absent | Stage 13.4 / 13.14 QEMU scripts | AHCI QEMU data-integrity test | QEMU only | High | IMPLEMENTED_UNVERIFIED |
| ATA PIO | `ata.rs` (11 KB) | Legacy PIO disk | Legacy path; not part of M5 target | None found by name | n/a | QEMU only | Low | IMPLEMENTED_UNVERIFIED |
| Unified block layer | `block.rs`, `block_io.rs`, `block_cache.rs`, `partition.rs`, `gpt.rs` | `BlockDevice` trait, `SECTOR_SIZE` constant, root selection (NVMe/AHCI/USB MSC) | Sector-size assumption is fixed constant; no common capacity/status/error-class API across NVMe, AHCI and USB MSC | Storage QEMU scripts | Multi-device coexistence test | QEMU only | High | PARTIAL |
| xHCI host controller | `xhci.rs` (126 KB) | Controller init, command/event/transfer handling, HID keyboard/mouse input, USB MSC transfers | **No hub support** (no matches for `hub` in `xhci.rs`); device re-enumeration, cancellation and controller error recovery not verified | Stage 13.12 / 13.13 / 13.18 QEMU scripts | Hub, re-enumeration, error-recovery tests | QEMU only | High | PARTIAL |
| USB HID | `hid.rs` (25 KB), `xhci.rs` | Boot-protocol keyboards and mice, report descriptor parsing | Untrusted-descriptor bounds not reviewed in this pass | `tests/hid_reports.rs`; Stage 13.13 / 13.16 QEMU scripts | Physical keyboard/mouse | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| USB mass storage | `usb_msc.rs` (8 KB), `xhci.rs` | Bulk-Only Transport, CBW/CSW validation, SCSI commands | Removal handling and stale-reference prevention unverified | `tests/usb_msc.rs`; Stage 13.18 QEMU script | Device removal, sector-size variation | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| UASP | none | None | Not started; not to be advertised | n/a | n/a | n/a | n/a | MISSING |
| VirtIO-net (legacy) | `virtio_net.rs` (28 KB), `hal/pci` | Legacy PCI I/O transport, split virtqueues, polling only | No MSI-X/interrupt-driven RX/TX; link status and ring-overflow handling unverified | Stage 10.x network QEMU scripts (`scripts/test-network-qemu.py`, `test-stage10-6-network.py`, `test-stage10-7-tcp.py`) | Ring-overflow, reset and statistics tests | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| Intel e1000 / e1000e | none | None | Not implemented | n/a | n/a | n/a | n/a | MISSING |
| Network stack | `network.rs` (133 KB), `async_network.rs` | IPv4/ARP/ICMP/UDP/TCP code paths; Stage 10.7 async TCP is the current accepted gate | Protocol coverage claims need per-protocol QEMU evidence | `tests/async_network.rs`; Stage 10.x QEMU scripts | Sustained-traffic and loss tests | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| Virtio modern transport | `virtio_pci.rs` (14 KB) | Modern virtio 1.x config for virtio-gpu | Not an M5 NIC target | Graphics scripts | n/a | QEMU only | Low | IMPLEMENTED_UNVERIFIED |
| Wi-Fi | `wifi.rs`, `wifi_hw.rs` (119 KB), `wifi_backend.rs`, `wifi_firmware.rs` (191 KB), `wifi_firmware_tlv.rs`, `wifi_ax200_image.rs`, `wifi_rsn.rs`, `wifi_wpa2.rs`, `wifi_gtk.rs`, `wifi_ccmp.rs`, and about 15 more `wifi_*` modules | Bounded backend contract, Intel TLV firmware-image validation, AX200/AX201 layout preflight, RSN/CCMP/GTK protocol code | No verified radio bring-up, scan, association or RX/TX on a physical chipset; firmware blobs are not in the repository; `wifi_backend.rs` documents that it claims no chipset support | `tests/wifi_tlv.rs`, `tests/wifi_firmware_tlv.rs`; Stage 13.9/13.10 gates | Chipset bring-up on real hardware (none available) | None | High | BLOCKED (physical); PARTIAL (software) |
| Bluetooth HCI | `bluetooth_hci.rs` (219 KB) | HCI command/event framing, LE lifecycle self-tests, carried over xHCI (per the xhci.rs reason string) | No pairing, GATT or audio profiles; ACL path not verified end-to-end | Stage 13.10 self-tests | End-to-end controller test | None | Medium | PARTIAL |
| Interrupt concurrency / IRQ locks | `irq_lock.rs`, `irq_mailbox.rs`, `interrupts.rs` | IRQ-safe lock wrapper and mailbox | Lock-order documentation for M5 drivers not written; storm protection absent | Not reviewed | Deterministic IRQ/concurrency stress | QEMU only | High | IMPLEMENTED_UNVERIFIED |
| ACPI | `hal/acpi.rs` (78 KB) | RSDP/SDT parsing with bounded table lengths, MCFG | Not reviewed in depth | None found by name | MCFG malformed-table tests | QEMU only | Medium | IMPLEMENTED_UNVERIFIED |
| Scheduler / allocator / VM | `task.rs`, `heap.rs`, `memory.rs`, `paging.rs`, `smp.rs` | Present and used by all drivers | Not audited in this pass | Stage 7 and 12 QEMU gates | n/a | QEMU only | n/a | IMPLEMENTED_UNVERIFIED |
| Physical hardware | none in repository | n/a | No physical driver run evidence was found in this audit | n/a | Physical M5 matrix | None accessible in this session | n/a | BLOCKED |

## 3. Existing host test coverage (relevant to M5)

Found: `tests/pci_assignment.rs`, `pci_bar.rs`, `pci_bridge.rs`,
`pci_bridge_transaction.rs`, `pci_msix_layout.rs`, `pci_resource.rs`,
`pci_routing.rs`, `pci_topology.rs`, `pci_vector.rs`, `usb_msc.rs`,
`hid_reports.rs`, `async_network.rs`, `wifi_tlv.rs`, `wifi_firmware_tlv.rs`.

Not found: any host tests for NVMe, AHCI, xHCI controller logic, virtio-net,
DMA ownership, or Bluetooth transport.

## 4. Build baseline (this audit)

| Command | Tree | Result |
|---|---|---|
| `cargo check -p wovenhat-kernel --target x86_64-unknown-none --features stage13-12-test` | `git archive HEAD` (temp) | exit 0 |
| same command | working tree | exit 101, 11 errors in `libwoven` (uncommitted, not ours) |
| `cargo check -p wovenhat-kernel --target x86_64-unknown-none` (no features) | working tree | exit 101 (same libwoven errors) |

Not run in this pass: host `cargo test`, warning-denying Clippy, QEMU boots,
`RUN-STAGE10.7.ps1`. The full gate was last recorded as passing on 2026-11-06
in `docs/stage-status.md`, before the current uncommitted changes.

## 5. Immediate findings for M5 planning

1. The M5 milestone is not in the repository roadmap. Its order and gates must
   be confirmed before implementation (AGENTS.md).
2. The working tree is not buildable until the libwoven owner resolves
   `OutlineOptions` scoping. M5 work cannot be validated on a broken tree.
3. Existing NVMe, AHCI, xHCI, virtio-net and Wi-Fi code is QEMU-tested
   only through the Stage 13 and 10 gates. No physical hardware evidence was
   found.
4. Highest-risk gaps: no IOMMU, no shared DMA API, no xHCI hub support, no
   e1000 driver, Wi-Fi unverified on any physical chipset.
5. Wi-Fi and Bluetooth cannot reach beyond PARTIAL or BLOCKED without a
   physical chipset and, for Wi-Fi, redistributable firmware.
