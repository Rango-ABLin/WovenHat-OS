# Stage 13.10D — Bluetooth Live USB HCI and Controller Initialization Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10D materializes the Stage 13.10B/C contracts into bounded xHCI-owned Bluetooth transport mechanics and a transport-independent controller initialization state machine.

Accepted behavior:
- endpoint-zero control-OUT-with-data path for HCI commands;
- Bluetooth E0/01/01 configuration enumeration and SET_CONFIGURATION;
- direction-aware xHCI DCI mapping;
- dedicated interrupt-IN event, bulk-IN ACL and bulk-OUT ACL endpoint contexts;
- separate rings and DMA buffers for Bluetooth event and ACL paths;
- bounded HCI event receive, ACL receive and ACL transmit primitives;
- Reset -> matching Command Complete -> Read Local Version -> matching Command Complete -> Ready sequencing;
- runtime marker `[S13.10D] Bluetooth controller initialization sequence: PASSED`.

## Authoritative validation

GitHub Actions run: 36747805657
Accepted source SHA: `41e009543e01afc6a348122ff3a2c4f0be16e351`
Release-validation job: 109998346871
Result: SUCCESS
Artifact: 11114005793
Artifact digest: `sha256:a245914f91143eb8f4d4b5aa6747d3d5bd07230c6dfa6d650455eb27ba52cae1`

The dedicated “Stage 13.10D Bluetooth controller init 1/2/4-core acceptance” step passed together with the full regression chain and evidence-preservation step.

## Boundary

This acceptance proves deterministic software architecture and xHCI/HCI ownership boundaries. The QEMU acceptance configuration does not provide a Bluetooth radio, so this audit does not claim physical Bluetooth-controller qualification. Real-device endpoint behavior, timing, hotplug, teardown/recovery and interoperability remain hardware qualification work.

## Next

Stage 13.10E should build Bluetooth controller capability discovery and device discovery/scanning above the accepted HCI initialization boundary. xHCI should remain transport-only; discovery policy and protocol parsing belong above it.
