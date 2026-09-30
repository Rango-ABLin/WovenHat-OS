# Stage 13.10F — Bluetooth ACL Connection and Link Lifecycle Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10F establishes bounded classic BR/EDR connection and ACL ownership above the accepted Stage 13.10E discovery boundary.

Accepted behavior:
- Create Connection command construction from a discovered-device record;
- Connection Complete validation and bounded live-link allocation;
- 12-bit controller-handle validation;
- Bluetooth-address/handle ownership pairing;
- idempotent identical connection completion;
- fail-closed conflicting handle or address reuse;
- Disconnect command construction and Disconnection Complete cleanup;
- bounded HCI ACL packet framing/parsing with a 1024-byte software payload ceiling;
- inbound and outbound ACL data restricted to currently owned handles;
- stale ACL authority rejected immediately after disconnect;
- malformed/truncated ACL data rejected.

## Authoritative validation

GitHub Actions run: 36761870548
Accepted source SHA: `79cecd7b0e3bab5db4426e2f6fed3eaa451c0a2b`
Release-validation job: 110046196796
Result: SUCCESS
Artifact: 11118843876
Artifact digest: `sha256:878f9d8866f09fd6e202062dbfa0784073ae7dbed5e196eb223387c26b40410c`

The Stage 13.10F Bluetooth ACL link lifecycle 1/2/4-core acceptance gate and evidence-preservation step passed.

## Boundary

This acceptance is deterministic software validation. It does not claim physical Bluetooth interoperability, controller ACL credit/flow-control integration, real-radio timing, hot-unplug, or transport recovery. Higher protocols such as L2CAP, pairing/security, ATT/GATT and profiles remain later stages.

## Next

Stage 13.10G should build bounded L2CAP framing and channel lifecycle on top of the owned ACL link authority established here.
