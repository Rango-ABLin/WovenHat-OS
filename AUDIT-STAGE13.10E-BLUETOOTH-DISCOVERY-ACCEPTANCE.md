# Stage 13.10E — Bluetooth Discovery and Controller Capability Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10E establishes bounded classic BR/EDR discovery and local-controller capability parsing above the accepted Stage 13.10D initialization boundary.

Accepted behavior:
- GIAC Inquiry command construction with bounded parameters;
- Inquiry Result and Inquiry Complete event parsing;
- bounded storage of 16 unique discovered Bluetooth addresses;
- retained page-scan repetition mode, class-of-device and clock offset;
- duplicate-device suppression;
- fail-closed malformed/truncated event handling;
- Read Local Version typed parsing;
- Read BD_ADDR typed parsing;
- Read Local Supported Commands 64-byte bitmap parsing;
- expected-opcode, controller-status and payload-length validation;
- runtime gates for Inquiry discovery and controller capability discovery.

## Authoritative validation

GitHub Actions run: 36753355665
Accepted source SHA: `98020bfbfa71dd68f0fc54b234761490947fe346`
Release-validation job: 110017294490
Result: SUCCESS
Artifact: 11115452902
Artifact digest: `sha256:dd94f81eed0b3916135051b78c0c27944a33084fba8405c1045dcf49e8196fe2`

The Stage 13.10E 1/2/4-core acceptance gate and evidence-preservation step passed. At the accepted SHA the runtime harness requires `[S13.10E] Bluetooth controller capability discovery: PASSED`, which executes after the Inquiry discovery self-test.

## Boundary

This acceptance is deterministic software validation. It does not claim physical Bluetooth-radio scanning or interoperability. BLE advertising reports, remote-name discovery, connection establishment, link teardown/recovery, L2CAP, pairing/security, ATT/GATT, hotplug and physical qualification remain open.

## Next

Stage 13.10F should consume bounded discovered-device records to establish and track classic Bluetooth ACL links, with explicit connection-handle ownership and disconnect/failure lifecycle before higher protocols are added.
