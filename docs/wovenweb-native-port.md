# WovenWeb native platform route

The target is a Ladybird-derived browser running inside WovenHat. A Windows/Linux
browser on the development machine is not native WovenHat execution. The current
native deliverable is a network platform SDK and real Ring-3 protocol probe, not
that finished browser. User priority permits this work alongside the unfinished
graphics roadmap; no later graphics stage is accepted by it.

## Port rather than replace the engine

Keep LibWeb/LibJS and their standards behavior. Implement a WovenHat platform
backend in AK/LibCore, then a native LibWebView UI backed by WovenUI surfaces.
Upstream Documentation/Porting.md requires separate platform and UI work. This
avoids a requirement to port the full Qt desktop stack, but does not eliminate
libc, C++ runtime, dependency, IPC, memory, graphics or sandbox work.

| Boundary | Existing WovenHat primitive | Required implementation / evidence |
| --- | --- | --- |
| Toolchain | Rust ELF loader and fixed native syscall ABI | Cross C/C++ sysroot, compiler runtime, libc/libc++, threading/TLS and exceptions policy; boot a C++ ABI fixture |
| Memory | mmap/munmap, file mappings, surfaces, shared memory | Real allocation/free, larger safe arenas, protections, shared-buffer lifetime/sealing and stress/teardown gates |
| Process | ELF loading, spawning, per-process handles | LibCore Process backend with explicit argv/environment/FD actions and renderer termination |
| Threads | Thread create/join, TLS registration | C++ TLS/runtime integration and scheduling/synchronization tests |
| Event loop | Generation-tagged completions, ports and event waits | LibCore EventLoopImplementationWovenHat using completions, timers and cancellation |
| Files | Descriptor reads/writes/seeks and VFS | LibCore System/File bridge, scoped profile/resource access, errors and metadata |
| IPC | Endpoint messages, service discovery and shared objects | LibIPC transport and handle transfer; do not pretend endpoint IDs are POSIX FDs |
| Networking | New owned async TCP SDK + SOCKS5 remote-name CONNECT | LibCore Socket/CURL adapter, deadline/cancellation tests, certificate-verified TLS and trusted time |
| Rendering | Shared surfaces, compositor, text/image substrates | LibWebView ViewImplementation and Application subclasses; input, resize, tabs, accessibility and frame damage |
| Sandbox | Capabilities, service/file/device policy | UI/network/renderer/image-decoder roles, no renderer raw NetworkIo, broker-only egress and hostile guest tests |
| Tor | SOCKS5 client transport now available | Native Tor daemon plus its TLS/crypto and runtime dependencies, bootstrap/control authentication and circuit isolation |
| Updates | WovenHat signed package/update primitives | WovenWeb packaging, upstream security tracking, signed releases and rollback |

The upstream vcpkg manifest pins OpenSSL/curl, ICU, HarfBuzz, compression and image
libraries, FFmpeg, Skia and other dependencies. Cross-build those dependencies;
changing a platform macro or renaming namespaces does not make them work.

## Memory blockers found in source

At the audited revision, config.rs limits anonymous mappings to 16 and shared
memory objects to 16 pages. userspace.rs uses USER_MMAP_STRIDE=0x10000 and rejects
anonymous mappings larger than that stride: 64 KiB per mapping, approximately
1 MiB total anonymous arena. libwoven's current allocator owns one 64 KiB arena
and does not reclaim individual allocations. These are concrete incompatibilities
with a modern browser, not build-flag problems. Rework bounded region management,
allocation/deallocation and shared buffers with ownership and OOM tests. Do not
simply raise constants or use the current bump allocator as a browser heap.

## First implemented platform boundary

libwoven::net now exposes owned TcpStream handles and borrowed Pending operations.
Sends are copied on submission; receives validate their original capacity before
kernel copyout. Pending work is neither Send nor Sync because completions are task-owned.
Drop cancels live operations and closes sockets, without exposing
raw constructors or keeping Ring-3 pointers in kernel work queues. poll supports
an asynchronous caller; wait blocks on completion events; wait_timeout uses a
completion port and a relative 100 Hz deadline. A timeout cancels the operation.
The legacy net::Socket token remains available for source compatibility.

libwoven::socks5 validates bounded ASCII DNS/punycode names, encodes ATYP=DOMAIN,
parses fragmented IPv4/IPv6/domain replies and refuses negotiation errors.
Only the explicit proxy endpoint is connected; no local name lookup or direct
target retry is implemented. No-auth SOCKS does not provide per-tab Tor circuit
isolation. The transport is not TLS and does not authenticate a proxy as Tor.
Deadlines are per operation; an overall request deadline and batched event-loop
registration remain work for the engine/network-service adapter.

The feature-isolated wovenweb-net-test boots an independently compiled Ring-3
probe. It checks network-authority denial, SOCKS remote name resolution, HTTP
content read into native DOM parsing, undersized receive refusal, repeated
cancellation, proxy refusal, timeout, and cleanup against pre-run counters.
The host captures packets and checks for IPv4 UDP DNS. This does not establish
IPv6/DNS-over-HTTPS leak resistance, real Tor connectivity or anonymity.

## Serverless-first network choice

The user has no VPN gateway. First target native Tor access without a VPN; the
Tor network supplies relays and exits, so a personally hosted VPN server is not
required. Port the Tor daemon on top of the same runtime/TLS work and restrict
browser network service egress to it. A desktop Tor process may be used as an
explicit development proxy only; do not market that as an in-OS Tor daemon.

WovenHat's future own VPN should use an established audited protocol such as
WireGuard and have a separate local test gateway. It still needs an independent
endpoint for public egress. Another home computer can test the tunnel but cannot
hide the home's public exit address. No endpoint, keys or operating VPN is
fabricated. Tor Project warns that adding a VPN can reduce anonymity if configured
incorrectly: https://support.torproject.org/tor-browser/general/vpn-with-tor/

## Ordered acceptance to a working browser

1. Accept native networking transport on 1/2/4 CPUs and preserve its evidence.
2. Accept native memory/runtime and a cross-built C++ ABI fixture, then AK tests.
3. Accept LibCore files, processes, IPC, timers and sockets; run helper processes
   inside the guest with cancellation, OOM, owner death and stale-handle tests.
4. Cross-build and test cryptographic/TLS/font/image dependencies, then LibJS and
   headless LibWeb inside WovenHat. TLS must verify certificates and hostnames.
5. Implement native LibWebView chrome and display real pages with links, forms,
   tabs, scrolling and download policy. Add the executable to the signed catalog.
6. Port native Tor, enforce broker-only egress, test onion/HTTPS/WS/download paths
   and failure containment with packet captures. Add a VPN only with a real endpoint.
7. Run web-platform, engine, sandbox, update and full OS acceptance before calling
   the result a full browser or asserting privacy properties.

Upstream references (source pinned by integrations/wovenweb/upstream.json):
Documentation/Porting.md, Libraries/LibCore/{System,Process,Socket,AnonymousBuffer,
EventLoopImplementation}.h, Libraries/LibWebView/{Application,ViewImplementation}.h,
AK/Platform.h and vcpkg.json.

## Experimental reclaiming heap follow-up (2026-10-25)

An opt-in reclaiming Rust allocator and native memory fixture are implemented.
Default SDK applications retain the legacy allocator. The bounded VM window
is unchanged; full runtime acceptance remains open because Clippy/fresh builds
were blocked by Windows Application Control and network/GUI regressions remain
incomplete. See [heap evidence and limits](wovenweb-heap-acceptance.md).

## WovenWeb regression follow-up (2026-10-25)

Fresh current-source heap, opt-in heap networking and default-allocator legacy
GUI matrices pass on 1/2/4 CPUs. Selected SDK/kernel/probe/GUI Clippy gates pass;
root workspace Clippy remains policy-blocked. The VM transaction state machine
is host-tested scaffolding only; production integration and larger VM limits
remain open. See wovenweb-heap-acceptance.md, wovenweb-heap-evidence.json and
wovenweb-vm-transactions.md. No native browser completion is claimed.

## Native C++ ABI fixture (2026-10-25)

A header-free Clang C++23/Rust Ring-3 probe passes on 1/2/4 CPUs with exact
resource cleanup. It covers integer/aggregate calls, guarded local static
initialization, aligned virtual objects and destructors; selected Clippy gates
pass. This is a controlled ABI fixture, not libc++, an engine build or full
runtime acceptance. Production VM integration remains open. Evidence and limits:
[Native C++ ABI fixture](wovenweb-cxx-acceptance.md).

## Pager ownership qualification (2026-10-25)

Production queued/active pager work now prevents exited-owner reaping, with
sequence-qualified completion and bounded reserved result capacity. Seven host
queue tests and the real-pager plus focused zombie fixture pass on 1/2/4 CPUs;
heap/network/C++ regressions also pass on all three CPU counts. Default and
affected kernel Clippy gates pass. Full VM transactions, exec/unmap/fork races,
thread-fault publication and native browser acceptance remain open. Local root
host checks remain signing-policy blocked; new hosted queue lint is pending.
See [pager lifetime evidence and limits](wovenweb-pager-lifetime.md).

## Sibling pager publication follow-up (2026-10-25)

The worker now publishes through retained process ownership instead of assuming
the faulting task is the leader. A real two-page sibling-thread fixture fails
the old lookup with zero canonical resident pages and passes the corrected
lookup with two, two completed faults and exact reaping/cache cleanup on 1/2/4
CPUs. Earlier portable Linux matrix dac3686e passed all three guest jobs, SDK
and Rust/PR CodeQL checks; this new source revision still needs hosted checks.
VM generations and exec/unmap/fork races remain open. See pager lifetime
evidence and limits in wovenweb-pager-lifetime.md.

## Physical handoff retention qualification (2026-10-25)

Production reaping now includes an outgoing process pin for every prepared CPU
context switch, released only on the incoming stack. Dead/Blocked metadata no
longer stands in for physical departure. The internal nonblocking exit observer
requires scheduler acknowledgement after dropping PROCESS_TABLE; syscall ABI
and process-info state encoding remain unchanged. An unchanged Stage7.4 assertion
first exposed the observer race; its failure log is retained. Six production
module host tests, metadata fixture plus real boot/pager/sibling tests and
heap/network/C++/legacy-GUI regressions pass on 1/2/4 CPUs. Default and affected
kernel lints pass. Hosted current-head qualification is pending; full VM journal,
wake/migration/thread-stack lifetime, exec/unmap/fork and browser work remain
open. See wovenweb-handoff-lifetime.md and wovenweb-handoff-evidence.json.

## Queued pager page-state qualification (2026-10-25)

Worker execution now refreshes canonical owner/root/file binding and publishes
only a single completed page delta, preserving unrelated page metadata. Already
resident queued duplicates complete without a second map. The old snapshot
overwrite is reproduced with one resident for two completed disjoint faults;
new disjoint/duplicate fixtures and metadata checks pass with exact cleanup on
1/2/4 CPUs. Heap/network/C++/legacy GUI regressions and selected lints also pass.
830d8886 hosted guest/SDK/CodeQL/core-foundation checks succeeded; this expanded
source awaits new hosted qualification. Same-file ABA, exec/raw-unmap/fork,
fast fault/eviction/cancellation recovery and complete VM journal admission
remain open, as do browser/runtime/Tor and full release acceptance. See
wovenweb-page-publication.md and wovenweb-page-publication-evidence.json.

## Mapping lineage source integration (2026-10-26)

Fresh mapping constructors now issue checked nonzero lineage IDs, included in
pager binding validation with existing owner/root identity. A real same-file
unmap/remap fixture and isolated Linux mutation control are added. This source
is NOT qualified yet: four Python parser checks pass; Rust and guest checks
await hosted CI because Windows now blocks clippy-driver and dependency build
scripts (4551). No policy bypass or repeated blocked invocation is attempted.
Initial fixture failures before worker release are retained without claiming
a reproduced stale fault. Current negative control must show precise acceptance
with only the tag comparison removed. ABI/capabilities/limits stay fixed; active
work lifetime and full VM journal integration remain open. See
wovenweb-mapping-generations.md and its evidence JSON.
