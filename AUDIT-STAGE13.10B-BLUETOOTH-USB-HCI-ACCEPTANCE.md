# Stage 13.10B — Bluetooth USB HCI Transport Contract Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10B establishes the USB Bluetooth HCI transport contract beneath the hardware-independent Stage 13.10A HCI core.

Accepted behavior:
- Bluetooth USB interface recognition for class/subclass/protocol E0/01/01.
- Interrupt-IN endpoint discovery for HCI events.
- Bulk-IN and bulk-OUT endpoint discovery for ACL traffic.
- Endpoint packet-size and interrupt-interval capture.
- Bluetooth class control-request setup construction for HCI commands.
- Deterministic descriptor and command-setup self-test.
- Stage 13.10 feature ownership of the existing xHCI PCI MMIO/bus-master helper.
- Runtime acceptance marker: `[S13.10B] Bluetooth USB HCI transport contract: PASSED`.

## Authoritative validation

GitHub Actions run: 36722602921
Accepted source SHA: `0ba3ce257f93775afe67ed975f8549fcdbb78506`
Release-validation job: 109911272075
Result: SUCCESS

The run passed lint and host regressions, the 1/2/4-core boot matrix, Stage 13.3 NVMe, Stage 13.4 AHCI/SATA, Stage 13.5 xHCI, Stage 13.6 USB HID, Stage 13.7 WovenInput, Stage 13.8 WovenAudio, Stage 13.9 WovenWiFi, and the Stage 13.10 runtime gate. The preserved release-validation artifact has digest `sha256:69a5693202ab9f8658d9a79e81da05a28a0fedb0fdd36b35d0a731df3e922805`.

The workflow step name still reads “Stage 13.10A Bluetooth HCI core 1/2/4-core acceptance”; this is a naming debt only. The runtime harness for stage 13.10 requires the Stage 13.10B marker, so the accepted run exercised the new contract.

## Acceptance boundary

This milestone does not claim a working physical Bluetooth controller. Dedicated xHCI endpoint contexts/rings for Bluetooth, DMA buffers, live HCI command transfer, interrupt event reception, ACL transfer execution, controller initialization/capability discovery, Bluetooth discovery, L2CAP, pairing/security, ATT/GATT, hotplug/lifecycle recovery, and physical hardware qualification remain later work.

## Next

Stage 13.10C should implement live USB HCI transfer execution and controller initialization while keeping USB ownership in xHCI and HCI protocol/state handling in `bluetooth_hci.rs`.
