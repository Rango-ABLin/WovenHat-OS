# Native WovenWeb network platform evidence - 2026-10-25

Accepted scope: owned SDK async TCP and bounded SOCKS5 transport against a local
fixture in the native guest. This is not a full native browser, native Tor daemon,
VPN, live onion/HTTPS verification or privacy certification.

The source route and remaining platform work are in wovenweb-native-port.md.
Machine-readable results, image/source/capture/log SHA-256 values and relative
evidence directories are in wovenweb-network-evidence.json. Raw audit artifacts
are preserved locally and intentionally ignored by Git.

## Executed evidence

- Native probe release ELF build: passed using its own native linker script.
- SDK suite: 166 unit tests and 3 compile-fail lifetime/thread-ownership tests passed.
- Repository host suite (`cargo test --locked`): 202 tests across 33 test binaries passed.
- Six packet-evidence unit tests passed, including truncated/unsupported capture
  rejection and distinguishing empty captures from positive TCP evidence.
- Workspace all-target Clippy, libwoven all-target Clippy, native probe Clippy,
  new kernel-feature Clippy and prior Stage 10.7/10.8 kernel-feature Clippy passed
  with `-D warnings`. Some compiler invocations reported an environment-only
  incremental-cache hard-link fallback; there was no source lint failure.
- Native guest gate passed on 1/2/4 CPUs with exit 33 and the same boot-image hash.
  Captures had 55/54/58 IPv4 TCP packets respectively and zero IPv4 UDP DNS packets.
- Guest cleanup returned to baseline: zero async operations, zero sockets,
  zero completion ports and the original two process-table entries.

The guest first proves that a Console-only user sandbox cannot create a network
socket. Its positive sandbox adds only NetworkIo. It negotiates SOCKS5 using a
remote hostname, receives a fragmented HTTP response, validates its body and
parses the HTML into the existing bounded DOM. It refuses a too-short receive
buffer, cancels 256 operations, rejects an authentication-required reply and
uses a completion-port deadline against a deliberately stalled method response.
No kernel syscall number or production authority policy was changed.

The preserved Stage 10.7 native async TCP gate also passed on 1/2/4 CPUs,
including its send/receive/EOF, pinning/cancellation and teardown assertions.

## Implementation and lifecycle

`libwoven/src/net.rs` owns native socket handles. Pending operations borrow their
socket, so safe Rust prevents socket destruction while operations are retained.
Pending work is also statically neither Send nor Sync, matching the kernel
completion owner task. The kernel copies sends at submission. Receive output is checked against the
original submitted capacity before copyout. Completion values, reserved fields,
event class/cookie/identity and one-shot consumption are checked. Pending Drop
cancels its generation; socket Drop closes it. Completion ports are scoped and
closed even on timeouts and errors. The kernel's existing pin/deferred-close
mechanism covers cancellation that races worker activity.

`libwoven/src/socks5.rs` validates the target before touching the network, sends
ATYP=DOMAIN and never invokes local DNS or a direct-target fallback. IPv4, IPv6
and domain response frames are bounded; fragmented and coalesced boundaries are
unit-tested. No-auth proxy access does not authenticate Tor or isolate circuits.
Blocking helpers are intended for a network worker; poll supports asynchronous
integration. Deadlines are per operation rather than an overall request budget.

`wovenweb-net-test` is feature-isolated. Its boot site and test module have their
own guards. Existing test-only queue/port counters admit the new feature without
changing production paths. The probe is independently linked and is not installed
in the production application catalog or presented as a browser application.

## Reproduce

From the root, with the pinned Rust toolchain, QEMU and its bundled OVMF:

```powershell
python scripts/test-wovenweb-network-qemu.py --cpus 1
```

Use the printed evidence directory's image.txt path as `--image PATH` for the
2- and 4-CPU runs. Run chains serially; the local host fixture owns port 18085.
A pass requires a successful host exchange, the guest markers, exit 33, positive
TCP capture evidence and no IPv4 UDP DNS packet in this controlled run.

## Limits and next prerequisites

The HTTP and DOM used here are bounded fixture substrates, not Ladybird's web
engine. These tests do not establish protection against arbitrary web content,
IPv6 leaks, DNS over HTTPS, external helpers, hostile proxies, renderer escapes,
certificate failures or fingerprinting. No claims about real Tor circuits,
anonymity, VPN operation or production installation follow from the fixture.
The full OS release and all other stage matrices remain separate qualification.

The next native platform boundary is memory/runtime: the OS's 64 KiB anonymous
mapping limit, 16-mapping arena and 64 KiB non-reclaiming SDK allocator cannot
support Ladybird's heap. Implement and qualify a real allocator/VM boundary,
cross-C++ runtime, LibCore backends, TLS and native LibWebView UI in the order
specified in wovenweb-native-port.md. The user's lack of a VPN server does not
block the Tor-first browser route.
