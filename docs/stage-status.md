# Stage status

## Stage 14.2D — Router Advertisement state (2026-10-06)

Router Advertisement state now tracks a link-local router, bounded lifetimes,
and one validated prefix with expiration and withdrawal. It rejects invalid
preferred/valid lifetimes and global router sources. Check the remote D audit
for its exact acceptance evidence.

## Stage 14.2E — ICMPv6 checksum boundary (2026-10-06)

The IPv6 foundation now has production-reusable address and prefix primitives:
prefix containment, link-local/multicast classification, and RFC 4291
solicited-node multicast derivation. The 1/2/4-CPU Stage 14.2 QEMU gate
exercises these exact operations and requires `[S14.2A]` and `[S14.2B]`
markers. This remains a deterministic foundation boundary: ICMPv6 Neighbor
Discovery packets, Duplicate Address Detection, router discovery, DHCPv6,
live IPv6 sockets, and physical IPv6 qualification remain open. See the
[Stage 14.2B audit](audit-stage14-2b-ipv6-neighbor-foundation-2026-10-06.md).

The bounded parser now accepts only ICMPv6 Router/Neighbor Solicitation and
Advertisement envelopes with code zero, valid fixed bodies, non-multicast
targets, and complete non-zero-length options. It caps input at 1 KiB and does
not retain packet ownership. The 1/2/4-CPU gate exercises valid and malformed
messages. IPv6 pseudo-header checksum verification, Duplicate Address
Detection, router state, DHCPv6, live IPv6 sockets, and physical IPv6
qualification remain open.

See the [Stage 14.2C audit](audit-stage14-2c-icmpv6-neighbor-parser-2026-10-06.md).

The parser now has a checked entry point that authenticates the ICMPv6
pseudo-header using the IPv6 source/destination, payload length, next-header
value, and packet checksum. Tampered packets are rejected before Neighbor
Discovery parsing. Router state, Duplicate Address Detection, DHCPv6, live
IPv6 sockets, and physical IPv6 qualification remain open.

See the [Stage 14.2E audit](audit-stage14-2e-icmpv6-checksum-2026-10-06.md).

## Stage 14.2F — Neighbor cache and DAD state (2026-10-06)

The bounded state layer now accepts only checksum-validated Neighbor
Advertisements, stores up to eight expiring link-layer records, and exposes
explicit Duplicate Address Detection pending/unique/duplicate outcomes. It
does not yet transmit probes or integrate live interface state. DHCPv6, live
IPv6 sockets, and physical IPv6 qualification remain open.

See the [Stage 14.2F audit](audit-stage14-2f-neighbor-state-2026-10-06.md).

Next: connect Neighbor Discovery state to bounded live IPv6 interface traffic.

## Stage 1–5 long-name mutation hardening — 2026-10-01

The FAT32 create/overwrite and rename paths now compare the user-visible long
name before selecting an 8.3 alias. Existing long-name files are overwritten
through their existing directory entry, and attempts to create a directory or
rename another file onto that name fail closed. The strengthened FAT32
self-test and complete Stage 1–5 QEMU gate passed on 1/2/4 CPUs with exit 33.
See [the hardening audit](audit-stage1-5-long-name-collision-2026-10-01.md).

This is a bounded software hardening increment, not a claim that the remaining
hardware qualification, DMA storage completion, or full data rollback gaps are
closed.

## Stage 1–5 durable data journal — 2026-10-01

FAT32 persistence now uses a bounded on-volume prepared/committed data journal
to recover interrupted single-file replacements, including restoration of the
previous bytes or removal of a newly-created target. Payload tampering fails
checksum validation. The journal self-test and 1/2/4-CPU QEMU gate passed.
See [the data-journal audit](audit-stage1-5-data-journal-2026-10-01.md).

Multi-file atomicity, larger-than-VFS files, and physical power-loss testing
remain open.

The journal now writes version 2 records with the intended replacement length
and checksum. Mount recovery validates committed records before retiring them
and restores the previous bytes if the committed target does not match.
Version 1 records remain readable for compatibility. Multi-file atomicity and
physical power-loss testing remain open.

## Stage 1–5 metadata capacity extension — 2026-10-01

The ownership sidecar preserves the WMD1 four-sector ABI and now uses a
versioned WMD3 extension in eight additional reserved sectors when the volume
geometry permits it. Capacity increases from 84 to 252 records without
relocating or rewriting legacy metadata. The strengthened self-test and
1/2/4-CPU QEMU gate passed with exit 33. See the
[metadata-capacity audit](audit-stage1-5-metadata-capacity-2026-10-01.md).

The table remains intentionally bounded; a dynamically growing metadata
format and full multi-operation transaction log are still future work.

Production completion is tracked separately from bounded foundation acceptance
in [the Stage 1–12 gap audit](stage1-12-production-gap-audit.md). No stage is
fully complete while a roadmap requirement remains open.

## Physical qualification tooling — 2026-10-01

The read-only physical-probe image now records CPU features, ACPI/ECAM state,
PCI class information, and every retained PCI function before stopping ahead
of driver activation. Its 1/2/4-CPU QEMU inventory smoke passed. This improves
lab evidence collection but does not close physical storage, DMA, interrupt,
radio, or unclean-shutdown qualification. See the
[physical inventory audit](audit-physical-inventory-2026-10-01.md).

Authoritative development sequence: [supplied Stage 10.7–36 roadmap](master-development-roadmap.md).

## TCP close/drain preservation repair - 2026-09-24

This-machine preflight exposed an intermittent 4-CPU Stage 10.7 failure:
last-reference release removed a TCP socket with 16 bytes still queued.
Graceful bounded retirement now preserves that transport. Ten corrected
4-CPU repeats passed, including queued-close cases; the full 75-boot Stage
10.7 preservation gate passed. Related 10.8/10.9/13.9 checks on 1/2/4 CPUs
and normal release shell/SMP smoke also passed. Assertions and existing test
timeouts are unchanged; cleanup now requires transport slots at baseline. See
[the TCP repair audit](audit-tcp-close-drain-2026-09-24.md).

## Physical integration request - 2026-09-24

Physical integration is incomplete. The latest user instruction selects this
HP Pavilion x360 as the actual test machine, superseding the earlier AX200
target choice. Its adapter is AX201 `8086:A0F0`, subsystem `8086:0074`, revision
`20`. Native AX201 startup and transport remain unimplemented; adding its PCI
ID to the AX200 activation list is not a valid implementation.

The authorized Kingston USB was cleared and written with the verified inventory
image. An elevated helper read back all 8,454,144 image bytes and matched the
SHA-256. Its new GPT layout contains an 8 MiB EFI system partition. The internal
system disk was not written. Secure Boot is enabled and the image has no PE
Authenticode certificate; native boot requires a local firmware decision.
Windows C: is fully encrypted with BitLocker protection on, so the recovery key
must be available before changing firmware security settings. Neither BitLocker
nor firmware settings were changed, and no reboot or physical test has run.
The inventory image passed 1/2/4-CPU configured QEMU boots plus a no-UART
framebuffer check. No AP startup or physical radio is tested by that mode.
See [the AX201 machine audit](audit-ax201-machine-2026-09-24.md).

The runtime firmware loader now validates the complete AX200 section layout
before DMA allocation and copies payloads into an unpublished owned context.
It skips separators and init-image records and preserves existing DMA bounds.
Build, warning-denying host/kernel/feature Clippy, all 35 Rust tests and all
54 Wi-Fi markers on 1/2/4 CPUs passed for this loader increment.
See [the runtime-loader audit](audit-ax200-runtime-loader-2026-09-24.md).

A real AX200 firmware container exposed parser defects. The corrected
allocation-free TLV parser passes seven new host tests and the real-image
probe; all 54 Wi-Fi markers still pass on 1/2/4 CPUs. Build, host-test and
kernel Clippy, and all 31 Rust tests pass. The full Stage 10.7 preservation
gate passed after the heap lint cleanup: nine Python tests and 75 QEMU
boots, plus three focused Wi-Fi boots (78 total for this prerequisite).
See [the physical preflight audit](audit-physical-wifi-preflight-2026-09-24.md).
This is prerequisite work, not physical firmware/IRQ/DMA/RF acceptance.

## Current continuation - 2026-09-24

Base commit `0563d0b` introduced the Stage 13.10AC deferred AX200 interrupt
service candidate. This continuation passed preservation and bounded QEMU
acceptance; it does not establish production completion of Stage 13.

- Ordinary build and host/freestanding-kernel Clippy passed with warnings
  denied; 24 Rust host tests and nine Python harness tests passed.
- The complete Stage 10.7 preservation gate passed, exit code 0: 75 QEMU boots
  covering 1/2/4 CPUs, live networking, and async block/file/UDP/TCP.
- Stages 1-5, 10.8-10.9, 11.1-11.5, 12.1-12.5, and 13.1-13.9 passed
  per-feature freestanding Clippy and 1/2/4-CPU QEMU (66 additional boots).
  Wi-Fi required all 54 markers through 13.10AC on every CPU configuration.
- CPU hotplug passed on 2/4 CPUs. The normal release build and 4-CPU
  shell/PS2/IOAPIC/SMP smoke passed. Total: 144 successful QEMU boots.

The continuation corrected feature boundaries, a misplaced audio check in
USB HID's error branch, and a nested Cargo release-build artifact-lock
conflict in the shell harness. Failed/interrupted attempts remain preserved.
See [the continuation audit](audit-stage13-10ac-continuation-2026-09-23.md)
for exact evidence directories and validation scope.

Next prerequisites remain production Wi-Fi worker/lifecycle integration,
secure entropy provisioning, physical AX201 firmware/IRQ/DMA/RF qualification,
and the earlier unfulfilled roadmap requirements. Do not advance to a later
stage or treat these synthetic Wi-Fi results as working physical Wi-Fi.

## Stage 6 — accepted on 2026-09-16

The bounded SMP foundation passed the complete release matrix: warning-denying
kernel/host Clippy, all Rust host tests, 1/2/4-CPU memory/storage/network
gates, legacy PIC fallback, 4-CPU release gates, and shell/SMP smoke. See
[the Stage 6 audit](audit-stage6-2026-09-16.md).

General multicore userspace, unrestricted concurrent device/filesystem service
throughput, multi-node NUMA page-placement qualification, non-contiguous
hotplug hardware qualification, APIC-ID-above-255 hardware qualification, and
broader cross-layer lock-path qualification and priority inheritance
remain explicit deferred requirements in the Stage 6 contract. CPU-domain
placement, audited Ready-state I/O service migration, and the x2APIC MSR transport are
implemented; bounded contiguous-prefix AP offline/re-online control is also
implemented; interrupt-safe scheduler, IPC, WovenGuard, teardown, and worker
locks now enforce bounded rank and nesting
checks, and VirtIO network DMA uses an allocator-reserved physically
contiguous arena; see the [NUMA/x2APIC audit](audit-stage6-numa-2026-09-16.md),
[hotplug audit](audit-stage6-hotplug-2026-09-16.md), and
[lock-order audit](audit-stage6-lock-order-2026-09-16.md) and
[DMA audit](audit-stage6-dma-2026-09-16.md).

The CPU lifecycle follow-up supports a non-contiguous online mask in QEMU:
CPU 1 can park while CPU 3 continues scheduling work and acknowledging TLB
shootdowns, then rejoin its stable slot. Offline preparation validates every
task move before committing any; draining CPUs reject new placement. See the
[non-contiguous hotplug audit](audit-stage6-noncontiguous-hotplug-2026-09-17.md).
Physical non-contiguous hotplug, high APIC-ID, and multi-node NUMA hardware
qualification remain open.
The final source passed the complete release matrix and dedicated twice-cycled
2/4-CPU hotplug gates after this follow-up.
The cancellation follow-up distinguishes unclaimed requests from AP-owned
transitions, rearms rejected states, and keeps further hotplug disabled if an
AP never finishes a claimed transition. Forced AP rejections and unclaimed
timeouts recover and retry in the 2/4-CPU QEMU gates; broad concurrent-work
and physical-fault stress remains open.

The 2026-09-17 lock follow-up made the file-frame cache IRQ-safe, moved
fork/dup reference retention outside the process-table lock, and added
generation-tagged VFS and pipe handles to reject stale slot reuse. The VFS
read and materialization paths now release registry/open-description guards
for disk I/O and revalidate node identity, version, backing, and shared seek
position before committing results. Both VFS tables now use ranked IRQ-safe
locks. See the
[lock-order audit](audit-stage6-lock-order-2026-09-16.md). The full release
matrix and twice-cycled 2/4-CPU hotplug gates passed after these changes.
The pipe follow-up also uses scheduler-latched wakeups and rejects full waiter
tables, closing a lost-wakeup window under concurrent readers and writers.
The global heap metadata lock is now rank-50 IRQ-safe; page mapping happens
before that guard is taken during boot. Its earlier full release and 2/4-CPU
hotplug gates passed. The follow-up makes boot capacity RAM-scaled from
256 KiB to 8 MiB, raises live-allocation metadata to 2,048 entries, recovers
alignment padding, and coalesces freed intervals. A partial kernel map now
rolls back. Runtime growth beyond the eager mapping remains open.
See the [Stage 6 heap audit](audit-stage6-heap-2026-09-17.md).
The final heap change passed the full release matrix and twice-cycled 2/4-CPU
hotplug gates.
The heap metadata follow-up replaces the fixed live-object and free-interval
tables with per-span headers and an in-place coalescing free list. Host tests
and a 4,096-object QEMU probe cover the former 2,048-object ceiling; mapped
runtime growth and broad allocator throughput remain open. See the
[heap metadata audit](audit-stage6-heap-metadata-2026-09-17.md).
The final metadata source passed the full 33-check release matrix and the
twice-cycled 2/4-CPU hotplug gates.
Runtime heap growth now maps and publishes 256 KiB chunks outside the heap
guard. Rank-free, IRQ-enabled allocations can grow on failure; BSP idle maps
ahead for guarded callers when free space is low. QEMU exercises growth after
SMP startup and reserve maintenance. Sudden large guarded allocations,
low-memory behavior, and multicore throughput still need qualification; see
the [heap growth audit](audit-stage6-heap-growth-2026-09-17.md).
The final growth source passed the complete 33-check release matrix and
twice-cycled 2/4-CPU hotplug gates.
The frame allocator now reserves a bitmap in each usable physical range and
uses it to track allocations, reject double frees, and recover returned
frames beyond its 4,096-entry hot cache. See the
[frame reclamation audit](audit-stage6-frame-reclamation-2026-09-17.md).
The QEMU memory gate exercises 4,352 returned frames; physical high-RAM and
NUMA latency qualification remains open.
The final frame-reclamation source passed the full 33-check release matrix
and dedicated twice-cycled 2/4-CPU hotplug gates.
The bounded device, keyboard decoder, journal, swap-state, mount-record, and
key-vault tables now use IRQ-safe locks. Keyboard input preserves the caller's
pre-lock interrupt state for early-boot polling; Stage 1-5 journal, Stage 12.3
key-vault, Stage 12.5 mount-record, and normal PS/2 shell gates passed.
The full release matrix and dedicated twice-cycled 2/4-CPU hotplug gates also
passed after the keyboard interrupt-state correction.
The FAT32 clean-page cache now uses a rank-20 IRQ mutex only for short metadata
and page-copy sections; physical page loads run after releasing it, with an
invalidation epoch preventing stale publication. Host, full release, and
2/4-CPU hotplug gates passed. Cross-layer FAT32 mutation transactions remain
open for unrestricted concurrent filesystem service.
The shell current-directory state now has a short rank-10 IRQ guard, with a
fixed-buffer copy released before console output or allocation. The terminal
has a rank-10 preemption guard that keeps device IRQs live during rendering,
and syscall writes restore live IRQs around full-frame operations. Lock-order
tracker updates remain interrupt-atomic. This closes the audited terminal and
shell compatibility-lock gap, but not the other Stage 6 production gaps above.
The final source passed the complete release matrix and twice-cycled 2/4-CPU
hotplug gates; details are in the lock-order audit.
The network runtime and VirtIO transport now declare ranks 20 and 30 for
their existing runtime-to-transport nesting. No kernel `IrqMutex::new` call
sites remain; this is rank coverage, not proof of every cross-subsystem path.
The ranked network change passed the full release matrix and twice-cycled
2/4-CPU hotplug gates.

## Stage 10.7 — accepted on 2026-09-16

The imported recovery source is preserved in Git commit `49e789d`. That commit
records the baseline only and does not certify acceptance.

The audit corrected worker starvation, local-preemption lock safety, readiness
observation, host-exchange validation and evidence retention. The complete gate
ended with `=== STAGE 10.7 ACCEPTANCE: PASS ===` and host exit status 0.

| Gate | Result |
| --- | --- |
| `cargo build` | Passed |
| Host and freestanding-kernel Clippy with `-D warnings` | Passed |
| Rust host regressions | 13 passed |
| Python host-harness regressions | 4 passed |
| Memory/scheduler/pager/IPC/security preservation | 1 CPU: 10/10; 2 CPUs: 30/30; 4 CPUs: 20/20 |
| Live DHCP/DNS/ICMP/UDP/TCP | 1/2/4 CPUs passed |
| Stage 10.4 asynchronous block I/O | 1/2/4 CPUs passed |
| Stage 10.5 asynchronous file I/O | 1/2/4 CPUs passed |
| Stage 10.6 asynchronous UDP | 1/2/4 CPUs passed |
| Strengthened Stage 10.7 asynchronous TCP | 1/2/4 CPUs passed |

Total: 75 QEMU boots passed in the final full chain. Each boot required debug-exit
status 33 and its required markers; network harnesses also verified host traffic.
Final build/lint/host checks also passed and are recorded under
`audit-artifacts/final-static/`. The accepted-stage commit follows the imported
baseline as a separate Git commit; use `git log --oneline` to identify it.

## Stage 10.8 — accepted on 2026-09-16

Completion ports, operation association, batch poll/wait, bounded reservations,
cancellation events, generation retirement and timeout-aware scheduler blocking
are implemented. The full gate passed with exit code 0: all 75 Stage 10.7 boots
plus completion-port boots on 1/2/4 CPUs (78 total). The latter include Ring-3
copyout retry, measured timeout, teardown, SMP producers and consumer wakeup.
Build, warning-denying host/kernel/probe Clippy, 18 Rust tests and four Python
harness tests passed. Final host tests include waiter notification and port quota.

Full transcript: `audit-artifacts/acceptance-20260916-085314-575/acceptance-output.txt`.
Focused serial evidence is retained in timestamped `audit-artifacts/stage10.8-*`
directories. See [ABI contract](completion-port-abi.md) and
[audit](audit-stage10-8-2026-09-16.md).

## Stage 10.9 — accepted on 2026-09-16

Timers, deadlines, asynchronous event objects, cancellation and sleep-until
are implemented under bounded process ownership. The 1/2/4-CPU QEMU gate,
build, warning-denying Clippy and Rust regressions passed. See
[audit](audit-stage10-9-2026-09-16.md).

## Stage 10.10 — accepted on 2026-09-16

The aggregate gate validates the Stage 10.8 completion architecture together
with Stage 10.9 timers/events on 1, 2, and 4 CPUs. All six boots passed with
exit 33 and retained serial evidence. Stage 10 is closed; Stage 11 follows.

## Stage 11.1 — accepted on 2026-09-16

PID lifecycle, parent/child ownership, exit status, wait semantics, process
groups, isolated resources and teardown passed build, Clippy, host tests, and
1/2/4-CPU QEMU validation. See [audit](audit-stage11-1-2026-09-16.md).

Next: Stage 11.2 per-process threads and join/TLS semantics.

## Stage 11.2 — accepted on 2026-09-16

Generation-safe thread IDs, owner-checked join, termination status, TLS and
thread-local errno state passed build, Clippy, host tests, and 1/2/4-CPU QEMU
validation. See [audit](audit-stage11-2-2026-09-16.md).

Next: Stage 11.3 structured notifications and lifecycle events.

## Stages 11.3–11.5 — accepted on 2026-09-16

Structured notifications, the `libwoven` userspace API boundary, and hardened
ELF/W^X loader validation passed their individual 1/2/4-CPU QEMU gates,
freestanding Clippy, and host tests. See [audit](audit-stage11-3-5-2026-09-16.md).

The follow-up ASLR pass now randomizes production ELF, stack and mmap bases
with a deterministic `qemu-test` switch; see [audit](audit-aslr-2026-09-16.md).

The 2026-10-05 loader-boundary follow-up rejects `PT_INTERP`, `PT_DYNAMIC`,
`PT_TLS`, and `PT_GNU_RELRO` program headers instead of silently accepting
binaries whose dynamic-linking, TLS, or RELRO semantics are not yet implemented.
This is a fail-closed production-safety boundary; dynamic relocations, shared
libraries, TLS image allocation, and RELRO permission transitions remain open.
See [audit](audit-stage11-loader-boundary-2026-10-05.md).

## Stage 12.1 — accepted on 2026-09-16

The VFS 2.0 typed interface and SystemVfs adapter passed build, freestanding
Clippy, and QEMU validation on 1/2/4 CPUs. See
[audit](audit-stage12-1-2026-09-16.md).

Next: Stage 12.2 WovenFS metadata and crash-consistency design.

## Stage 13.1 — accepted on 2026-09-16

The WovenDriver manager passed device matching, binding, suspend/resume,
build, freestanding Clippy, and 1/2/4-CPU QEMU validation. See
[audit](audit-stage13-1-2026-09-16.md).

## Stage 13.2 — topology/ownership increment implemented; full acceptance open (2026-09-27)

The PCIe foundation now includes bounded generation-tagged function/bridge
topology, explicit single-owner claims, generation-tagged logical MMIO BAR
leases, teardown-before-reuse invalidation, and bridge bus-window parentage.
The dedicated preservation gate requires focused host tests, warning-denying
kernel Clippy, runtime-harness regressions, and 1/2/4-CPU QEMU evidence.

Full Stage 13.2 acceptance remains blocked on BAR size probing/allocation and
rebalance, bridge I/O/MMIO/prefetchable resource routing, generic MSI/MSI-X
vector allocation/programming/teardown, PCIe hotplug, and WovenDriver
binding/unbinding integrated with generation ownership. The current paging
layer also lacks a physical MMIO unmap/revoke primitive; topology teardown
therefore revokes logical lease authority but must not be described as page
table mapping revocation.

Stage 13.3 NVMe is intentionally not advanced by this work.

## Stages 12.2–12.5 — accepted on 2026-09-16

WovenFS metadata/integrity, volume-integrity envelope boundaries, snapshots,
and storage-management inventory each passed their 1/2/4-CPU gates. See
[audit](audit-stage12-2-5-2026-09-16.md). Production AEAD now passes the
isolated 1/2/4-CPU gate with a bounded revocable key vault; see
[audit](audit-volume-crypto-2026-09-16.md). Measured key provisioning,
persistent key storage, rotation policy, and encrypted-volume mount integration
remain open before encrypted volumes are security-complete.

## Stage 7.1.5 - accepted on 2026-09-16

The scheduler/pager watchdog and termination lifecycle gate passed in full:
50 one-CPU memory runs, 100 two-CPU runs, 50 four-CPU runs, and live
DHCP/DNS/ICMP plus host-verified UDP/TCP at 1/2/4 CPUs. See
audit-stage7-1-5-timeout-2026-09-16.md for the bounded lifecycle handoff
correction and retained serial evidence.

## Roadmap Stage 13.10A — Bluetooth HCI core accepted on 2026-09-30

The hardware-independent Bluetooth HCI foundation is accepted. Bounded command encoding, Command Complete parsing, controller command-credit state, opcode/status validation, and deterministic self-tests passed the dedicated 1/2/4-CPU QEMU gate with exit 33 in GitHub Actions run 36718370038. See [acceptance audit](../AUDIT-STAGE13.10A-BLUETOOTH-HCI-ACCEPTANCE.md).

Historical Wi-Fi labels 13.10A–AC are Stage 13.9 AX200/WovenWiFi extensions; roadmap Stage 13.10 is Bluetooth. Stage 13.10A does not claim USB transport or physical Bluetooth hardware support.

Next: Stage 13.10B Bluetooth USB HCI transport over xHCI.


## Roadmap Stage 13.10B — Bluetooth USB HCI transport contract accepted on 2026-09-30

The xHCI-owned Bluetooth USB transport contract is accepted at the descriptor/protocol boundary. Stage 13.10B recognizes the USB Wireless Controller / RF Controller / Bluetooth Primary Controller interface (E0/01/01), validates the interrupt-IN HCI event endpoint plus bulk-IN/bulk-OUT ACL endpoints, and constructs the class control-request setup packet used to carry HCI commands. The hardware-independent `bluetooth_hci.rs` protocol layer remains separate from USB transport semantics.

GitHub Actions run 36722602921 passed at source SHA `0ba3ce257f93775afe67ed975f8549fcdbb78506`, including lint/host regressions, the 1/2/4-core boot matrix, Stage 13.3–13.9 regression gates, and the Stage 13.10 runtime gate. The runtime harness requires `[S13.10B] Bluetooth USB HCI transport contract: PASSED`; the workflow step label still says Stage 13.10A and should be renamed in the next acceptance-maintenance change.

This acceptance is a transport-contract milestone, not a physical-controller claim. Live xHCI endpoint/ring ownership, HCI command/event transfer execution, ACL transfer execution, controller initialization, discovery, L2CAP, pairing/security, ATT/GATT, lifecycle/hotplug, and physical hardware qualification remain open.

Next: Stage 13.10C live USB HCI transfer execution and controller initialization boundary.


## Roadmap Stage 13.10C — bounded HCI transfer execution contract accepted on 2026-09-30

The Bluetooth command/event transaction boundary is accepted. The HCI core now serializes one outstanding command against controller credits, rejects a second command while one is in flight, requires the matching Command Complete opcode, and restores readiness only after successful completion. The xHCI Bluetooth layer also exposes bounded command, interrupt-event, ACL-IN, and ACL-OUT transfer plans derived from the accepted Stage 13.10B interface/endpoint contract.

GitHub Actions run 36729724615 passed at source SHA `5aa5628ad815ef2f04e0e1b3a150dad0bc30b355`. The dedicated “Stage 13.10C Bluetooth HCI transfer 1/2/4-core acceptance” gate and the complete Stage 13.3–13.9 regression chain passed. Preserved release-validation artifact digest: `sha256:b9bb2fa1d58274d268f069b44440844adbb87e51f3a04e0547d8868b5080dcf0`.

This milestone does not yet claim a physical Bluetooth controller or live USB transfer completion. Dedicated xHCI Bluetooth endpoint contexts/rings and DMA buffers, live control-OUT HCI command submission, interrupt-IN event reception, ACL execution, Reset/Read Local Version against hardware, teardown/recovery, and physical qualification remain open.

Next: bind the accepted transfer plans to dedicated xHCI rings/DMA ownership and execute the controller-initialization sequence.


## Roadmap Stage 13.10D — live USB HCI mechanics and controller initialization accepted on 2026-09-30

Stage 13.10D materializes the Bluetooth USB transport contract as xHCI-owned state: endpoint-zero control-OUT-with-data for HCI commands, dedicated interrupt-IN event and bulk-IN/bulk-OUT ACL endpoint contexts, separate transfer rings and DMA buffers, direction-aware DCI mapping, and bounded event/ACL execution primitives. The hardware-independent HCI layer adds a fail-closed controller initializer that requires Reset Command Complete before Read Local Version and reaches Ready only after the matching second completion.

GitHub Actions run 36747805657 passed at exact source SHA `41e009543e01afc6a348122ff3a2c4f0be16e351`. The dedicated “Stage 13.10D Bluetooth controller init 1/2/4-core acceptance” step passed together with the complete regression chain. Preserved release-validation artifact digest: `sha256:a245914f91143eb8f4d4b5aa6747d3d5bd07230c6dfa6d650455eb27ba52cae1`.

This acceptance proves the bounded transport/initialization architecture under deterministic CI; it does not claim physical Bluetooth-radio qualification because the QEMU acceptance device is not a Bluetooth controller. Physical USB-controller execution, teardown/recovery, hotplug, and radio qualification remain explicit hardware work.

Next: Stage 13.10E Bluetooth discovery/scanning and controller capability discovery above the accepted HCI initialization boundary.


## Roadmap Stage 13.10E — Bluetooth discovery and controller capability contract accepted on 2026-09-30

Stage 13.10E adds bounded BR/EDR GIAC Inquiry discovery above the accepted HCI initialization boundary. It constructs the Inquiry command, parses Inquiry Result and Inquiry Complete events, retains up to 16 unique devices by Bluetooth address, and records page-scan repetition mode, class-of-device, and clock offset. Malformed/truncated events fail closed and duplicate addresses do not consume additional discovery slots.

The HCI layer also parses controller identity/capability responses for Read Local Version, Read BD_ADDR, and Read Local Supported Commands. Opcode, controller status, and response length are validated before exposing HCI/LMP version data, manufacturer/subversion, local Bluetooth address, or the 64-byte supported-command bitmap.

GitHub Actions run 36753355665 passed at exact source SHA `98020bfbfa71dd68f0fc54b234761490947fe346`, including the Stage 13.10E 1/2/4-core acceptance gate and evidence preservation. Release-validation artifact digest: `sha256:dd94f81eed0b3916135051b78c0c27944a33084fba8405c1045dcf49e8196fe2`.

This remains deterministic software validation; physical Bluetooth-radio scanning is not claimed. BLE advertising reports, remote-name discovery, connection establishment, L2CAP, pairing/security, ATT/GATT, lifecycle/hotplug, and physical interoperability remain later work.

Next: Stage 13.10F Bluetooth connection establishment and bounded link lifecycle above the accepted discovery state.


## Roadmap Stage 13.10F — Bluetooth ACL connection and link lifecycle accepted on 2026-09-30

Stage 13.10F consumes bounded discovery records to construct classic BR/EDR Create Connection commands, validates Connection Complete and Disconnection Complete events, and owns up to eight live ACL links by 12-bit controller handle and Bluetooth address. Duplicate identical completion is idempotent; conflicting handle/address reuse fails closed.

The HCI layer now frames and parses bounded ACL packets (1024-byte software ceiling), validates handle/packet-boundary/broadcast fields, and permits inbound/outbound ACL data only for currently owned live handles. Disconnect immediately revokes ACL authority, so stale-handle traffic is rejected.

GitHub Actions run 36761870548 passed at exact source SHA `79cecd7b0e3bab5db4426e2f6fed3eaa451c0a2b`, including Stage 13.10F 1/2/4-core acceptance and evidence preservation. Artifact digest: `sha256:878f9d8866f09fd6e202062dbfa0784073ae7dbed5e196eb223387c26b40410c`.

This remains deterministic software validation. Physical controller timing, ACL flow-control credits, asynchronous transport recovery and real peripheral interoperability remain hardware/production qualification work. Next: Stage 13.10G L2CAP framing and bounded channel lifecycle above owned ACL links.


## Roadmap Stage 13.10G — Bluetooth L2CAP accepted on 2026-09-30

Stage 13.10G adds bounded L2CAP Basic Mode framing above the Stage 13.10F owned ACL link. L2CAP length/CID parsing is exact, CID zero is rejected, payload storage is fixed-capacity, and every frame remains anchored to a live ACL handle.

The signaling layer implements bounded Connection Request/Response and Disconnection Request/Response state with dynamic local CIDs beginning at 0x0040 and a fixed eight-channel table. Established channels own the tuple of ACL handle, PSM, local CID and peer CID. Outbound data targets only the peer CID; inbound data is accepted only for the owned local CID on the same live ACL handle. Teardown revokes channel authority immediately.

GitHub Actions run 36785295506 passed at exact source SHA `c2dff3c12c9c9387a10493900b3846489e8539a6`, including the Stage 13.10G 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:6ac9ff255444ffa36128cd03d75f3d2c89eff086e308618ed21fed463c4a8eab`.

This is deterministic software acceptance. Configuration negotiation, fragmentation/reassembly, enhanced modes, controller flow control and physical interoperability remain outside this boundary. Next: Stage 13.10H pairing, authentication and encryption/security.


## Roadmap Stage 13.10H — Bluetooth pairing/authentication/encryption accepted on 2026-10-01

Stage 13.10H adds a fail-closed BR/EDR security boundary above the accepted ACL/L2CAP ownership layers. A live ACL link progresses explicitly through unauthenticated, authenticating, authenticated, encryption-enabling and secured controller states. Authentication or encryption failure does not grant secured authority.

The stage also adds bounded link-key ownership, Link Key Request positive/negative replies, Link Key Notification ingestion, explicit key revocation, and policy-mediated Secure Simple Pairing interactions for IO capability, numeric confirmation and passkey requests. Pairing confirmation is never silently auto-approved.

Trusted secured authority requires all of: a live owned ACL handle, the same peer address owning a stored link key, successful controller authentication, and controller-confirmed encryption. Removing the key or disconnecting the ACL link makes the trust predicate fail closed.

GitHub Actions run 36791065311 passed at exact source SHA `dda2933eef36026ab70cefbe85898bcd62256f06`, including the Stage 13.10H Bluetooth security 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:f731dbd2f52dc7c2743aa052eaa7f801eef99dc0fc6050f13e6fa71518a07405`.

This is deterministic software acceptance. Persistent secure key storage, full controller event orchestration, physical-radio pairing/interoperability and BLE security remain outside this boundary.


## Roadmap Stage 13.10I — Bluetooth LE foundation accepted on 2026-10-01

Stage 13.10I establishes a bounded Bluetooth Low Energy controller/discovery and connection-lifecycle foundation without conflating LE identity or security with the accepted BR/EDR link-key model. The HCI layer can configure scanning, enable or disable scanning, parse LE Meta Advertising Reports, retain public/random peer address type, capture bounded legacy advertising data and RSSI, and deduplicate repeated reports by address type plus address.

The connection layer consumes an LE discovery record to build LE Create Connection, accepts exact LE Connection Complete events into a fixed eight-link ownership table, rejects malformed, conflicting or out-of-range handle/address state, and removes ownership on Disconnection Complete. A disconnected LE handle is immediately stale and cannot retain authority.

GitHub Actions run 36810460362 passed at exact source SHA `edcfe31b5cd1406c4feb2d001dbc0e2d2f6d7a9f`, including the Stage 13.10I Bluetooth LE foundation 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:f14abafba66e1a599c2410e7e458c3cd2447ca783780deb688afffbfb26b5251`.

This is deterministic software acceptance. Physical LE controller/radio interoperability, extended advertising, LE privacy/resolving lists, connection-update orchestration, ATT/GATT and BLE SMP/bonding remain outside this boundary. Next: Stage 13.10J ATT/GATT foundation above owned LE ACL links.


## Roadmap Stage 13.10J — Bluetooth ATT/GATT foundation accepted on 2026-10-01

Stage 13.10J builds a bounded ATT/GATT software contract above live Stage 13.10I LE link ownership. ATT read/write transactions reject stale LE handles, malformed PDUs, invalid attribute handles and permission violations. The attribute database is fixed-capacity and uses nonzero handles with bounded values.

The GATT layer represents 16-bit Primary Service and Characteristic Declaration/value attributes, preserves ATT permission enforcement, supports bounded service/characteristic discovery by handle range, and models per-LE-link notification/indication subscriptions. Subscription emission rechecks the live LE handle and exact characteristic value handle; unsubscribe or disconnect revokes authority.

GitHub Actions run 36815325948 passed at exact implementation SHA `48bdd085e8d8918bb058a59de06d42ce8ad7108a`, including the Stage 13.10J 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:9247004a999b8470e5431a2396d1d1237bd1cd278f43abdf9ebf9460ed0a3dca`.

This is deterministic software acceptance. Physical BLE ATT/GATT interoperability, ATT MTU negotiation, 128-bit UUIDs, full wire-level discovery procedures, indication confirmations, BLE SMP/bonding and protected persistent keys remain outside this boundary. Next: Stage 13.10K BLE profiles and services above the accepted ATT/GATT authority model.


## Roadmap Stage 13.10K — BLE profiles and services acceptance candidate on 2026-10-01

Stage 13.10K builds bounded BLE services above the accepted Stage 13.10J ATT/GATT authority model. Standard profile support includes Device Information with read-only Manufacturer Name and Model Number characteristics plus Battery Service with a range-checked read-only Battery Level.

Battery updates can emit Handle Value Notifications only through an existing per-link, per-value-handle GATT subscription. Failed authorization, invalid levels, wrong handles or stale LE links leave the prior battery state unchanged. The stage also adds a bounded WovenHat OS service with a read-only status characteristic and write-only command characteristic, preserving ATT permission checks and live-link authority.

The accepted profile checkpoint run 36819330823 passed at exact source SHA `c5f3c6194c58ad16e75c0f44cf90c6d336393c40`. Preserved evidence digest: `sha256:677f88e58b878de73553225ec533f76c4d1b0e74864b1a69aaa30367caef50db`.

Closure hardening makes characteristic creation transactional with respect to ATT table capacity: two slots are preflighted before declaration/value insertion, preventing a capacity failure from leaving an orphan declaration.

This remains deterministic software validation. Physical BLE profile interoperability, standardized conformance testing, BLE SMP/bonding, persistent protected keys and production controller recovery remain outside this boundary. Next after final exact-head acceptance: Stage 13.10L Bluetooth lifecycle and hardening.


## Roadmap Stage 13.10L — Bluetooth lifecycle and hardening acceptance candidate on 2026-10-01

Stage 13.10L hardens the accepted BLE software authority model around teardown, reset and reconnect. `BluetoothLeLifecycle` coordinates live `LeLinkState` with `GattSubscriptions`: a valid disconnect removes the link and revokes every subscription owned by that handle, while controller reset clears all LE links and subscriptions. A monotonic wrapping generation records successful lifecycle invalidation.

Malformed disconnect events and unknown/stale handles fail closed without changing links, subscriptions or generation. Reconnecting a peer does not inherit its previous subscription authority; explicit re-subscription is required before notification emission is authorized.

The bounded stress gate executes 32 connect/subscribe/notify/teardown cycles, alternating normal disconnect with controller reset. Every cycle must end with zero live links, zero subscriptions and stale notification rejection. The accepted stress checkpoint run 36825376710 passed at exact SHA `4c0b0e5e0c8d3b9e00b47dc58868a154c9c970ef`; preserved evidence digest `sha256:67cc6f35203061ef8a79dfba3a8fef739db667bf7fb907b1eb989659c104f9e0`.

This acceptance remains deterministic software validation. Physical Bluetooth controller/radio recovery, transport fault injection, BLE SMP/bonding/LTK security, persistent protected keys, radio interoperability and conformance certification remain outside this boundary.


## Roadmap Stage 13.10R — BLE secure-session identity binding accepted on 2026-10-01

Protected LE ATT/GATT operations now require the transient secure session to
match the live link's peer address type and address as well as its connection
handle. This rejects stale authorization when a controller reuses a handle for
a different peer, including outbound notifications and indications. The exact
tip passed host tests, warning-denying kernel Clippy, and the Stage 13.10
1/2/4-CPU QEMU gate with the identity-binding marker.

This remains deterministic software validation. Physical BLE controller/radio
interoperability, transport fault injection, BLE SMP/bonding/LTK security,
persistent protected keys, radio interoperability and conformance
certification remain outside this boundary.


## Stage 13.11A — BLE SMP pairing/fixed-channel foundation — 2026-10-05

The BLE Security Manager Protocol foundation validates bounded Pairing Request
and Response parameters, enforces supported key-size and distribution flags,
and accepts SMP frames only on fixed L2CAP CID `0x0006` for a live LE link.
Pairing negotiation is handle-bound and fail-closed on malformed or stale
traffic. The dedicated 1/2/4-CPU QEMU gate passes locally; GitHub acceptance
for the pushed tip remains required.

Cryptographic key derivation, user interaction, passkey/numeric comparison,
bonding persistence, controller transport and physical BLE qualification remain
outside this foundation.


## Stage 13.11B — BLE SMP LTK/key distribution — 2026-10-05

Legacy Encryption Information and Master Identification PDUs are now validated
and integrated with the canonical LE bond store. Distribution is bound to the
live pairing handle, negotiated key size and peer identity; malformed, stale,
duplicate and mismatched inputs fail closed. The dedicated 1/2/4-CPU QEMU
gate passes locally; GitHub acceptance for the pushed tip remains required.

Cryptographic derivation, user-confirmed pairing, controller transport and
physical BLE qualification remain outside this bounded distribution stage.

The distribution closure additionally binds pending LTK material to peer
identity, rejects duplicate pending material, and rejects completion after
connection-handle reuse.


## Stage 13.11C — BLE SMP confirm/random verification — 2026-10-05

Pairing Confirm and Pairing Random verification is now handle-bound and tied to
the negotiated pairing parameters and peer address context. Confirm mismatches
transition the bounded state to failure; malformed or out-of-order messages
fail closed. The dedicated 1/2/4-CPU QEMU gate passes locally; GitHub
acceptance for the pushed tip remains required.

This is a protocol/crypto boundary, not complete production pairing. User
confirmation UX, authenticated key-derivation integration, bonding
persistence, controller transport and physical BLE qualification remain open.


## Stage 13.11F — BLE controller encryption/session authority — 2026-10-05

The SMP flow now owns the LE Start Encryption command boundary and binds
Encryption Change completion to the pending handle, peer identity, key size and
authentication state. Failed, disabled, stale or mismatched events fail closed
and revoke authority. The dedicated 1/2/4-CPU gate passes locally; GitHub
acceptance for the pushed tip remains required.

Physical controller transport, SMP interoperability, bond persistence and
hardware qualification remain outside this deterministic software boundary.


## Stage 13.11G — BLE identity/privacy foundation — 2026-10-05

The SMP layer now validates identity distribution, stores bounded IRK-backed
peer identities, and resolves private addresses only against retained identity
material. Duplicate, malformed and stale inputs fail closed, while pending LTK
distribution remains bound to the live peer identity. The dedicated 1/2/4-CPU
gate passes locally; GitHub acceptance for the pushed tip remains required.

Physical privacy-list behavior, SMP interoperability, persistent protected keys
and hardware qualification remain outside this deterministic foundation.


## Stage 13.10S closure hardening — 2026-10-05

Minimum BLE encryption-key-size policy is enforced for both protected ATT/GATT
requests and outbound notifications/indications. Weak authenticated sessions
are rejected before either inbound data access or outbound value encoding;
sessions meeting the configured threshold remain bound to the live peer
identity. The exact 1/2/4-CPU Stage 13.10 gate, host tests and warning-denying
kernel Clippy passed locally. GitHub acceptance for the source tip is required
before this closure is considered complete.


## Stage 13.10T — outbound BLE key-size acceptance contract — 2026-10-05

The Stage 13.10 runtime harness now requires a dedicated marker for outbound
notification authorization by minimum encryption key size. A weak authenticated
session is rejected before notification encoding, while a session meeting the
16-byte policy is accepted. The local 1/2/4-CPU gate, host tests and
warning-denying kernel Clippy pass; GitHub acceptance for the pushed tip is
still required.
