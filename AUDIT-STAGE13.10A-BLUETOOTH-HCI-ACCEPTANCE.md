# Stage 13.10A Bluetooth HCI Core Acceptance

Accepted: 2026-09-30

Roadmap Stage 13.10 is Bluetooth. Historical Wi-Fi labels named Stage 13.10A–AC belong to the Stage 13.9 AX200/WovenWiFi development line and do not represent roadmap Bluetooth completion.

## Accepted scope

Stage 13.10A establishes the hardware-independent Bluetooth HCI command/event foundation in `kernel/src/bluetooth_hci.rs`.

The accepted slice provides bounded HCI command encoding, payload validation, Command Complete parsing, controller command-credit state, opcode/status validation, and deterministic self-tests. The kernel acceptance marker is:

`[S13.10A] Bluetooth HCI core foundation: PASSED`

The feature boundary is `stage13-10-test` at both workspace and kernel levels.

## Acceptance evidence

Authoritative GitHub Actions run: 36718370038
Accepted source SHA: 94565ad73b7401445ca62701b0c4f62e637cfdfe
Job: 109896881823

Dedicated Stage 13.10A acceptance passed on:
- 1 CPU, exit 33: `audit-artifacts/stage13.10-1cpu-1790773606550489976`
- 2 CPUs, exit 33: `audit-artifacts/stage13.10-2cpu-1790773610193015780`
- 4 CPUs, exit 33: `audit-artifacts/stage13.10-4cpu-1790773614608965589`

The same run also passed the release validation chain and Stage 13.3 through Stage 13.9 preservation gates.

## Architectural boundary

This acceptance does not claim a Bluetooth hardware driver or usable Bluetooth connection. Stage 13.10A intentionally does not activate xHCI. USB Bluetooth transport, controller capability discovery, ACL transport, discovery/scanning, L2CAP, pairing/security, ATT/GATT/BLE services, lifecycle/hotplug, and physical hardware qualification remain later work.

Next roadmap slice: Stage 13.10B, Bluetooth USB HCI transport over the existing xHCI foundation.
