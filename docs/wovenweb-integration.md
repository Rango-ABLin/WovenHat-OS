# WovenWeb integration audit - 2026-10-25

Status: experimental host source foundation. Not a native OS application or a
release acceptance. Requested by the user independently of the active graphics
roadmap; that roadmap remains at its existing incomplete Stage 07.

## Architecture and source relationships

`integrations/wovenweb/upstream.json` pins Ladybird and `ladybird.patch` carries
six source changes: LibHTTP proxy selection, WebView privacy defaults/CLI help,
Qt window branding, and TestProxy regressions. `prepare.py` fetches exactly that
commit into a new directory, verifies it, applies the patch and optionally builds
using the upstream driver. No existing checkout is reset.

`wovenweb.py` launches only explicit HTTP(S) inputs through a local SOCKS endpoint.
It configures proxy-side DNS and no bypass list, a fresh temporary profile,
JavaScript off, disk cache off, UTC and local starter block rules. It never
passes arbitrary browser switches supplied as URLs and never falls back to a
direct launch after failed SOCKS negotiation. See the integration README for
remaining fingerprinting, IPC/settings, DNS, external-handler and crash risks.

The OS's `libwoven::browser`, `libwoven::web` and WovenUI WebView are independent
bounded native substrates. None is replaced by this host fork. There is no
native Qt/POSIX port, application-catalog entry, VPN device or working VPN
endpoint added by this change. WovenGuard is a firewall, not a VPN.

## Validation observed in this checkout

- 11 Python launcher tests passed, including actual closed-loopback refusal,
  fragmented/truncated/invalid SOCKS replies, hostile proxy variables, rejected
  schemes/arguments, launch refusal and child exit-code propagation.
- Python launcher and preparation script byte-compilation passed.
- Patch applies with `git apply --check --cached` against the pinned upstream
  index; upstream and OS `git diff --check` passed.
- `cargo test -p libwoven --locked`: 154 unit tests passed.
- `cargo clippy -p libwoven --all-targets --locked -- -D warnings`: passed.
- Browser build attempted: upstream driver reports that a Visual Studio enabled
  environment is required. CMake/Ninja/C++ compiler are not on this session PATH.
- WSL inspection returned Wsl/E_ACCESSDENIED. No browser executable was installed.

Rust dependency downloads used workspace-local CARGO_HOME and CARGO_TARGET_DIR
because the global Cargo cache was not writable. No secrets were read or stored.
C++ TestProxy was added but could not be compiled or executed. Full OS build,
workspace/kernel Clippy and QEMU 1/2/4-CPU acceptance have not been completed for
this branch. No native/runtime/browser security gate is accepted by these tests.

## Blocking prerequisites and next acceptance

1. Build/run the patched browser in a supported Linux/WSL2 or configured native
   Windows environment and execute TestProxy. Validate HTTP(S), WS(S), downloads,
   certificate AIA and redirect paths through real Tor, including failure tests.
2. Add OS-level egress containment and capture IPv4/IPv6/DNS traffic during Tor
   failure, reconnect, hostile environment/settings, and external-handler tests.
3. For WovenHat's own VPN, select an audited protocol, provision a gateway/public
   endpoint, store keys outside Git, implement native tunnel/routing and DNS,
   then verify rekey/reconnect and kill-switch behavior. No custom crypto.
4. Port the required Ladybird platform services, sandbox and Qt UI to native
   WovenHat; only then add a real executable to the application catalog and pass
   the full repository build/Clippy/host/QEMU matrix.
5. Establish maintained tracker lists, fingerprinting review, signed updates and
   independent security review before making production protection claims.

No browser can promise universal untrackability. A successful SOCKS greeting
neither authenticates Tor nor proves a bootstrapped circuit or working VPN.
