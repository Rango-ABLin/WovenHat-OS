# Stage 13.10G — Bluetooth L2CAP Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10G adds a bounded L2CAP Basic Mode and dynamic-channel ownership layer above the accepted Stage 13.10F ACL link lifecycle.

Accepted behavior:
- exact L2CAP Basic header length and CID parsing;
- rejection of CID zero and malformed/truncated frames;
- fixed-capacity L2CAP payload storage;
- L2CAP frames bound to an owned live ACL handle;
- Connection Request/Response signaling;
- dynamic local CID allocation beginning at 0x0040;
- bounded eight-channel ownership table;
- Disconnection Request/Response and deterministic channel reclamation;
- outbound data restricted to an established channel's peer CID;
- inbound data restricted to the established local CID on the same ACL handle;
- stale channel transmit/receive authority rejected immediately after teardown.

## Authoritative validation

GitHub Actions run: 36785295506
Accepted source SHA: `c2dff3c12c9c9387a10493900b3846489e8539a6`
Release-validation job: 110125141796
Result: SUCCESS
Artifact: 11129384616
Artifact digest: `sha256:6ac9ff255444ffa36128cd03d75f3d2c89eff086e308618ed21fed463c4a8eab`

The Stage 13.10G Bluetooth L2CAP framing 1/2/4-core acceptance gate and evidence-preservation step passed.

## Boundary

This acceptance is deterministic software validation. It does not claim L2CAP configuration negotiation, segmentation/reassembly, enhanced retransmission/streaming modes, controller ACL credit integration, physical-radio timing, or peripheral interoperability.

## Next

Stage 13.10H should add bounded Bluetooth pairing/authentication/encryption state while preserving the ACL and L2CAP ownership boundaries established in 13.10F/G.
