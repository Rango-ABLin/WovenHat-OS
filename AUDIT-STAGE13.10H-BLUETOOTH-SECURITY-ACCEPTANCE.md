# Stage 13.10H — Bluetooth Pairing, Authentication and Encryption Acceptance

Accepted: 2026-10-01

## Scope accepted

Stage 13.10H adds deterministic BR/EDR link-security authority above the accepted Stage 13.10F ACL and 13.10G L2CAP ownership boundaries.

Accepted behavior:
- per-handle authentication/encryption state progression;
- Authentication Requested and Authentication Complete handling;
- Set Connection Encryption and Encryption Change handling;
- failures do not grant secured authority;
- bounded link-key ownership and replacement;
- Link Key Request positive reply for known peers and negative reply for unknown peers;
- Link Key Notification ingestion and explicit key revocation;
- IO Capability Request reply;
- User Confirmation Request parsing with explicit accept/reject;
- User Passkey Request parsing with bounded six-digit input and explicit rejection;
- Simple Pairing Complete success/failure parsing;
- trusted-secured authority requires the live ACL handle, matching peer identity with a stored link key, successful authentication, and controller-confirmed encryption;
- key removal and ACL teardown revoke trusted authority fail closed.

## Authoritative validation

GitHub Actions run: 36791065311
Accepted source SHA: `dda2933eef36026ab70cefbe85898bcd62256f06`
Release-validation job: 110143823518
Result: SUCCESS
Artifact: 11131973476
Artifact digest: `sha256:f731dbd2f52dc7c2743aa052eaa7f801eef99dc0fc6050f13e6fa71518a07405`

The Stage 13.10H Bluetooth security 1/2/4-core acceptance gate and evidence-preservation step passed.

## Boundary

This acceptance is deterministic software validation. It does not claim persistent encrypted key storage, full asynchronous HCI security orchestration, physical Bluetooth radio pairing/interoperability, BLE SMP, or hardware-backed key protection. The controller remains responsible for Bluetooth cryptographic link operations; WovenHat validates and gates authority from the resulting HCI state.

## Next

Continue Stage 13.10 with the BLE foundation / LE advertising and connection model while preserving the existing ACL, L2CAP and security ownership boundaries.
