# Stage 14.6 live socket firewall admission audit

Stage 14.6 integrates the bounded Stage 14.5 policy evaluator with the shared
smoltcp socket runtime. Connect, send, and receive decisions run while the
existing runtime lock protects socket owner/generation state and firewall
rules. Pinned async network calls enter the same socket functions and therefore
share the same decisions.

Outbound TCP checks use the chosen ephemeral local port before calling
`smoltcp::Socket::connect`, preventing a local-port selector from being
bypassed by the initial SYN. UDP connect binds only after policy allows the
candidate local port. UDP ingress checks the received source endpoint; TCP
ingress checks the active local/remote endpoints before exposing bytes to the
caller. Socket ownership and WovenGuard `NetworkIo` syscall checks are
unchanged.

The standalone policy object defaults to deny. The live runtime has an
explicit allow fallback to preserve the accepted network behavior until
kernel-managed policy is configured. New explicit rules precede older rules.
Policy mutation has kernel functions but no userspace syscall or management
authority plumbing yet.

## Admission structure

Policy selectors are derived once per operation by `firewall_selectors`, which
reads live socket state rather than duplicating the derivation at each call
site. `firewall_allows` makes the decision, `firewall_record_denial` commits it
to the ledger, and `firewall_admit` composes the two. The six socket entry
points — synchronous and pinned `connect`/`send`/`recv` — share these, so a
future selector or protocol change has one place to be correct.

### Denied TCP ingress revokes the connection

An ingress denial on an established TCP connection aborts that connection and
reports the terminal `SocketError::Address`. Returning `WouldBlock` instead
would leave the received bytes queued in the socket buffer, keep the peer's
connection open against a pinned receive buffer, and — because
`async_network::process_one` re-queues a `WouldBlock` request while
`can_recv()` remains true — spin the asynchronous worker against a condition
that cannot clear. A policy denial is permanent for the life of the connection,
so it is treated as terminal, consistent with the existing warning in
`socket_connect` against reporting a permanent condition as `WouldBlock`.

UDP ingress deliberately differs. A datagram socket may receive from many
peers, so a denial drains and discards exactly one datagram and returns
`WouldBlock`, leaving the socket usable for permitted peers. The datagram is
consumed before the decision, so a denied sender cannot wedge the queue.

One related behavior change falls out of deriving selectors before the send.
`socket_send` on a TCP socket with no resolvable remote endpoint — never
connected, or fully closed — now returns `NotConnected`. Previously it reached
`smoltcp::Socket::send_slice`, whose `InvalidState` error was mapped to
`WouldBlock`, so the asynchronous worker retried a permanently dead socket.
Connected and accepted sockets are unaffected, because `remote_endpoint()` or
the recorded peer resolves for both.

### Denials are auditable

Every refused socket operation commits one `audit::Action::WovenGuardDeny`
record through the Stage 9.3 security ledger: the owning task as `actor`, the
packed refused endpoint as `target`, and the direction, protocol and local port
packed into `detail`. The ledger lock is rank 40 and the network runtime lock
is rank 20, so recording from inside a runtime-guarded path increases rank and
cannot invert lock order.

Only denials are recorded. The ledger is a bounded 128-entry ring; logging
authorized packets as well would evict the denial history that makes the record
useful. The UDP connect path evaluates up to sixteen ephemeral candidates, so
it evaluates policy per candidate but records at most one denial per refused
connect — the Stage 14.6 self-test asserts exactly that, by requiring the
ledger sequence to advance by one.

### Non-IPv4 endpoints take the configured default

Rule selectors are IPv4-only, so an IPv6 endpoint cannot match any rule. Such
an endpoint takes the table's configured default action rather than an
unconditional deny. An unconditional deny would contradict the runtime's
documented contract that no installed policy preserves existing behavior, and
would silently break IPv6 sockets when that path is implemented; routing
through the default keeps the contract true while still letting an
administrator fail IPv6 closed by choosing a deny default. IPv6 selectors are
later work.

## Acceptance results

- Warning-denying kernel Clippy passed for every declared kernel feature:
  39 configurations (default plus all 38 features in `kernel/Cargo.toml`).
- `cargo clippy --all-targets -- -D warnings` (host) — passed.
- `cargo test` — passed; all host unit and harness test targets.
- `python scripts/test-network-qemu.py --feature stage14-6-test --cpus {1,2,4}`
  — passed for 1, 2, and 4 CPUs with exit 33, including live DHCP/DNS/ICMP and
  host-verified UDP/TCP round trips. All 20 Stage 14 markers `[S14.1]` through
  `[S14.6]` were present in each run.
- Serial logs: `target/network-regression-{1,2,4}-debug/serial.log`.

### Prior-stage gates repaired by this change

Three Stage 14 and two Stage 12 configurations did not compile at
commit `c53d597`, so their prior-stage gates were unrunnable. This was
confirmed by linting that commit in a detached worktree.

- `stage14-2-test`, `stage14-3-test`, `stage14-4-test` failed because the
  Stage 14.5 self-test call in `kernel/src/main.rs` had no
  `#[cfg(feature = "stage14-5-test")]` guard, unlike its 14.2/14.3/14.4
  siblings, while the function itself is feature-gated.
- `stage12-3-test` and `stage12-5-test` failed on a circular module gate:
  `storage_manager` (gated on 12.5) seals records through `volume_crypto`
  (gated on 12.3), while the Stage 12.3 self-test block calls into
  `storage_manager`. Both modules are now gated
  `cfg(any(feature = "stage12-3-test", feature = "stage12-5-test"))`.

The Stage 12 features deliberately do **not** chain the way the Stage 14
features do. Chaining was tried first and the gate rejected it: each Stage 12
self-test block ends the boot with `qemu_test_exit_success()`, so making
`stage12-5-test` enable `stage12-3-test` caused the 12.3 block to exit the boot
before the 12.5 block ran. The 12.5 gate then failed on all three CPU counts
with `[S12.3] encryption: PASSED` and no `[S12.5]` marker. Shared module
dependencies between Stage 12 gates therefore belong on the module's `cfg`, not
in the feature graph; `kernel/Cargo.toml` records that constraint.

All five configurations now lint clean, and their QEMU gates pass on 1, 2 and
4 CPUs. The complete Stage 12.1–12.5 chain was re-run on 1/2/4 CPUs to confirm
that each stage still reaches its own marker.

## Scope limits

This deterministic software gate does not prove packet-level TCP handshake
filtering, ICMP/IPv6 or kernel-owned traffic filtering, physical firewall
enforcement, connection tracking, NAT, or physical NIC qualification.

The denied-TCP-ingress abort is exercised by review and by the terminal-error
contract, not by a live denied TCP connection in QEMU: the gate's policy rules
target a synthetic endpoint so the live DHCP/DNS/ICMP/UDP/TCP regressions stay
unaffected. A live denied-TCP-ingress boot remains worthwhile follow-up.
