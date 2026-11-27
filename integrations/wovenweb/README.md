# WovenWeb experimental host foundation

This is source integration, not a released browser, native WovenHat executable,
working VPN, or anonymity certification. The Ladybird engine is pre-alpha.
Keep upstream LICENSE and copyright notices in every source/binary distribution.

## Implemented

- Reproducible Ladybird commit pin and patch; no mass copy of upstream into the OS.
- WovenWeb window titles and CLI help. Internal Ladybird namespaces, platform IDs,
  licensing, and executable target names remain for upstream compatibility.
- New-profile defaults: Global Privacy Control on, content blocking on with blocked
  count visible, background networking off. Existing saved preferences may override
  defaults. Subscription updates are off; the launcher loads local starter rules.
- Opt-in WOVENWEB_TOR_ONLY=1 overrides proxy environment variables and NO_PROXY
  with socks5h://127.0.0.1:9050 for HTTP(S)/WebSocket configuration. Hostnames are
  resolved through SOCKS rather than locally in upstream request paths.
- Host launcher refuses unavailable/invalid SOCKS endpoints, uses a temporary
  profile, disables JavaScript and disk HTTP cache, and selects UTC.
- Original, small local tracker-domain list. It does not block every tracker.

## Build on a supported host

Install Ladybird prerequisites from its pinned Documentation/BuildInstructionsLadybird.md.
Linux or Windows WSL2 is recommended; native Windows remains experimental.
Python 3.10+ and Git are required for these scripts.

```sh
python integrations/wovenweb/prepare.py /absolute/new/path/WovenWeb --build
python integrations/wovenweb/wovenweb.py --binary /absolute/path/WovenWeb/Build/release/bin/ladybird https://example.com
python -m unittest discover -s integrations/wovenweb -p 'test_*.py' -v
```

Use a trusted, independently installed Tor daemon listening on 127.0.0.1:9050
in the SAME host/network namespace as the browser. Check Tor bootstrap separately.
A SOCKS greeting only establishes protocol availability; it cannot authenticate
Tor, verify the circuit, or establish that an onion service is reachable.
Tor binaries, relay infrastructure and subscription downloads are not bundled.
The launcher must point to the patched build; it cannot attest arbitrary binaries.
Use explicit http:// or https:// onion URLs. No public gateway is involved.

Temporary profiles are removed on clean shutdown, not guaranteed after crashes;
downloaded files, swap, system logs, and external applications may retain data.
The browser can still be fingerprinted. GPC is a preference signal, not a blocker.
JavaScript-off breaks many sites and does not eliminate tracking. Site settings
can change during a session. System-level egress enforcement has not been built
or tested. No protection against a compromised host is claimed.

## WovenHat native integration boundary

WovenHat is a custom Rust OS, not Linux. Its existing bounded DOM parser and
HTTP endpoint parsing do not provide Ladybird's Qt, C++ runtime, POSIX process,
thread, IPC, shared memory, filesystem, TLS/crypto, font, GPU and sandbox APIs.
The fork therefore cannot be added to the native launcher catalog as a working
application yet. This directory is a host-side integration boundary only.
No kernel ABI or accepted stage is modified. See docs/wovenweb-integration.md.

## WovenHat's own VPN

A browser proxy is not a VPN. WovenGuard packet filtering is not a VPN protocol.
There is no native VPN implementation or server endpoint configured here.
Use an audited protocol such as WireGuard; do not invent encryption.
The required work is native tunnel/packet routing, audited protocol support,
key storage and rotation, a self-hosted gateway with a reachable public endpoint,
DNS routing, IPv4/IPv6 egress enforcement, reconnect/rekey handling, and kill-switch
acceptance. Private keys must never enter this repository.
For a Tor-over-VPN deployment the Tor process, not just the browser, must use the
VPN route. VPN-down must block Tor egress before reconnecting. This requires
OS-level policy and packet-capture verification; a browser flag cannot provide it.

## Required release evidence

Build and run the C++ TestProxy regression; run live HTTP(S)/WS(S), DNS, TLS AIA,
redirect, download and onion tests. Capture IPv4/IPv6 traffic while Tor is stopped,
restarted and misconfigured; test hostile NO_PROXY, IPC/settings changes and
external handlers. Add independent sandbox/fingerprinting review, supply-chain
verification, signed updates and maintained tracker lists. Native OS acceptance
requires the repository's full build/Clippy/host/QEMU gates on 1/2/4 CPUs.
No such release or native guest acceptance is claimed by this foundation.

## Native platform continuation

The OS now has an owned async TCP/SOCKS5 SDK and a real native userspace probe,
qualified against a controlled fixture on 1/2/4 CPUs. See
../../docs/wovenweb-network-acceptance.md and ../../docs/wovenweb-native-port.md.
This does not make the Ladybird host fork a native browser executable. The user
has no VPN server; native Tor access is prioritized as the first network route.
