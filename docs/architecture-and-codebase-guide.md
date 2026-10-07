# WovenHat OS Architecture & Codebase Guide

## TCP close/drain repair (2026-09-24)

A closed TCP descriptor retains its transport after the final async reference
until graceful close completes. `network::poll` reaps those existing bounded
slots; a 30-second grace expires on a subsequent poll for unresponsive peers.
Slots remain counted and unavailable for reuse until retirement. This fixes
queued-data loss when a send completion was consumed before transmission.
See [the TCP repair audit](audit-tcp-close-drain-2026-09-24.md) for failure
evidence, locking/ownership review and validation limits.

## Selected physical machine and inventory boot (2026-09-24)

The latest user instruction selects this HP Pavilion x360 with AX201
`8086:A0F0`, superseding the earlier AX200 target preference. Native AX201
transport is not implemented. The existing AX200 activation boundary remains
unchanged until the appropriate device-specific implementation is reviewed.

`physical-probe` is an independent root/kernel Cargo feature. After early
RAM and paging checks, `kernel_main` enters `physical_probe::run`, enumerates
PCI through the existing HAL, prints network identities and AX201 BAR/capability
details to the framebuffer and serial, and halts. It does not enter driver
activation, storage/network runtime, AP startup or candidate QEMU tests. No
device configuration or MMIO register writes are added by this mode; PCI
configuration reads use the existing ECAM/legacy address-selection access.
It is an inventory aid and never radio acceptance. See the
[AX201 machine audit](audit-ax201-machine-2026-09-24.md).

## Physical Wi-Fi firmware prerequisite (2026-09-24)

The preceding increment targeted AX200; AX201 support is not enabled.
`wifi_ax200_image.rs` preflights runtime LMAC/UMAC/paging region ordering,
payload size and total DMA capacity. `Intel22000DmaContextInfo::from_runtime_firmware`
uses that plan to copy payloads into independently owned DMA buffers. Failure
drops the unpublished partial context; success still requires queue setup and
a reviewed physical publication/lifecycle owner. See the
[runtime-loader audit](audit-ax200-runtime-loader-2026-09-24.md).

`wifi_firmware.rs` delegates Intel container parsing to `wifi_tlv.rs`, an
allocation-free, immutable-borrow parser shared with host tests. It validates
a bounded complete blob, exposes paging-size metadata separately, and
streams runtime/init SEC records including secure variants and explicit
CPU/paging separators. The DMA stager rejects separators before allocation.
This closes real-container parsing failures; it does not implement the
physical transport. The inspected host is AX201, while the candidate match
is AX200. See [the physical preflight audit](audit-physical-wifi-preflight-2026-09-24.md)
for firmware hashes, validation and the remaining lifecycle/transport work.

## Stage 13.10AC integration boundary (2026-09-23)

The sections below describe previously recorded foundations. The current
Wi-Fi candidate is additional, feature-gated source: `main.rs` includes
`wifi_hw` and the other Wi-Fi modules only with `stage13-9-test`. It does not
yet provide the normal boot path with a working physical AX200 driver.
The session and smoltcp adapter now share that boundary. `network.rs` keeps
the candidate transport selector in a feature-gated module, while ordinary
builds use `VirtioSmolDevice` directly. This corrects the previously inconsistent
module graph without changing the VirtIO packet path.

`interrupts.rs` installs the reserved PCI Wi-Fi vector `0xd0`. Its hard IRQ
publishes an atomic work flag and acknowledges the LAPIC. `wifi_hw.rs` exposes
`service_deferred_ax200_interrupt`, which consumes that flag and calls
`IntelRxInterruptController::service` outside the hard IRQ. The controller
masks CSR delivery, reads interrupt status, acknowledges enabled RX causes,
and restores its configured mask; fatal hardware/firmware status returns an
error with delivery masked. The flag coalesces notifications and is not a
counted queue or a scheduler wakeup. The only current deferred-service caller
is the synthetic self-test. A production worker, device lifecycle ownership,
teardown/recovery, and physical interrupt/DMA qualification remain open.

The synthetic CSR test uses an owned DMA page and verifies written values;
ordinary RAM does not emulate write-one-to-clear hardware semantics. The
MSI discovery branch explicitly skips physical programming when AX200 is
absent. Neither synthetic success nor that skip proves physical Wi-Fi works.

`scripts/test-stage10-runtime.py` now pins all 54 Wi-Fi acceptance markers
through 13.10AC in addition to the SMP and async ABI markers and exit code 33.
`tests/test_runtime_harness.py` checks complete evidence, each missing Wi-Fi
marker, the former incomplete marker set, and unsuccessful QEMU exit.
The Stage 13.9 launcher uses the same project-local Cargo directories as
the Stage 10.7 preservation launcher.

## Candidate feature boundaries (2026-09-24)

Ordinary builds retain the VirtIO network transport, PCI discovery and legacy
I/O bus-master path, the reserved Wi-Fi interrupt vector, and PS/2 keyboard
input. Candidate-only APIs are compiled alongside their existing callers:

- The Wi-Fi secure-pool candidate and MSI programming module use
  `stage13-9-test`; the normal best-effort network RNG is unchanged. The
  deterministic test pool is not a production cryptographic entropy source.
- PCI MMIO bus-master enablement follows the existing NVMe, AHCI, xHCI, HDA,
  and Wi-Fi candidate features. Two uncalled private configuration-write
  wrappers were removed; driver-used configuration transactions are retained.
- Audio registration uses `stage13-8-test`, matching `woven_audio` and HDA.
- Extended input event candidates and their self-test use `stage13-7-test`.
  No non-keyboard producer exists in the ordinary build. Its byte input ABI,
  bounded queue, overflow accounting, and IRQ-safe locking are unchanged.
- Wi-Fi deferred-work consumers and BSP MSI destination selection follow the
  Wi-Fi candidate feature. The installed interrupt handler is unchanged.

These boundaries do not supply missing production drivers. They add no
syscalls, capabilities, kernel objects, locks, or asynchronous ownership paths.
Existing candidate acceptance tests remain enabled under their original
features; no new lint suppression was introduced.

The Stage 13.6 HID report-error branch now reports the HID error directly;
a duplicated HDA discovery block was removed. Stage 13.8 retains its original
audio codec/topology checks. The normal-release shell harness isolates the
bootloader dependency's nested Cargo installer in `target/bootloader`, while
an explicit parent `--target-dir` keeps the main artifact tree unchanged.
This avoids the parent/child release artifact-lock deadlock. The validated
results and physical-driver limits are recorded in the continuation audit.

## Scope and source of truth

This guide describes the Stage 10.7 source, not the proposed 1.0 system. WovenHat
currently has a monolithic Rust kernel, a UEFI boot image builder, embedded Ring-3
programs, and host/QEMU acceptance harnesses. The supplied
[master development roadmap](master-development-roadmap.md) defines future stages.
[Stage status](stage-status.md) records which gates have actually passed.

## Repository map

| Files | Responsibility |
| --- | --- |
| `Cargo.toml`, `kernel/Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml` | Workspace, pinned toolchain, freestanding kernel artifact and feature selection |
| `build.rs`, `src/main.rs` | Package the kernel with the UEFI bootloader; expose the generated image path to harnesses |
| `kernel/src/main.rs`, `config.rs` | Initialization order, subsystem wiring, configuration bounds and boot acceptance orchestration |
| `hal/`, `ap_start.S`, `smp.rs`, `gdt.rs`, `interrupts.rs`, `pic.rs`, `timer.rs` | CPU/platform discovery, AP startup, descriptor tables, interrupts and timekeeping |
| `task.rs`, `userspace.rs`, `elf.rs`, `syscall.rs` | Scheduling, process lifecycle, Ring-3 images, ELF validation and syscall dispatch |
| `memory.rs`, `paging.rs`, `heap.rs`, `swap.rs`, `file_frames.rs`, `file_mapping.rs`, `page_cache.rs` | Physical/virtual memory, allocation, backing storage and file mappings |
| `block.rs`, `block_cache.rs`, `block_io.rs`, `ata.rs`, `partition.rs`, `gpt.rs`, `storage.rs` | Block devices, request execution, caching, partitions and storage initialization |
| `vfs.rs`, `fat32.rs`, `async_file.rs` | Filesystem operations and positional asynchronous file requests |
| `async_op.rs`, `async_network.rs`, `irq_lock.rs` | Completion ownership, asynchronous sockets and interrupt-safe network/completion locks |
| `network.rs`, `virtio_net.rs` | smoltcp IPv4 sockets and the transitional VirtIO-net transport |
| `capability.rs`, `wovenguard.rs`, `audit.rs`, `entropy.rs` | Authority, domains, resource policy, security ledger and entropy support |
| `ipc.rs`, `pipe.rs` | IPC objects, endpoint/message operations and byte pipes |
| `device.rs`, `keyboard.rs`, `serial.rs`, `console.rs`, `terminal.rs`, `shell.rs` | Device registry, input, diagnostics and command interfaces |
| `graphics.rs`, `gui.rs` | Existing framebuffer/GUI support; not the future compositor |
| `panic.rs`, `benchmark.rs` | Failure reporting and existing benchmark support |
| `tests/`, `scripts/`, `run-stage*-acceptance.ps1` | Host regressions, QEMU orchestration and stage preservation gates |

## Stage 6 filesystem concurrency boundary

`vfs.rs` keeps node metadata and shared open descriptions in separate rank-10
IRQ mutexes; when both are needed, open descriptions precede nodes. Handles
carry non-wrapping slot epochs, and node references carry generations. Disk-
backed reads and lazy materialization snapshot identity, backing path, version,
and (for shared reads) offset, drop both VFS guards for ATA I/O, then revalidate
before publishing data or changing the seek position. A conflict retries a
bounded number of times. Rename increments node versions. Prefix iteration
copies bounded path names before calling external code. File-frame cache
updates may nest under the VFS guards at rank 10.

The global heap's metadata guard is rank 50 and disables local interrupts
during allocation and deallocation. Boot-time heap page mapping occurs before
acquiring that guard, preserving the paging (10) to heap (50) order. Its eager
mapped size is one sixteenth of available physical memory, clamped to 256 KiB
through 8 MiB. The allocator retains alignment padding with each allocation
and coalesces freed intervals. Kernel range mapping rolls back newly mapped
pages on a partial failure. Runtime growth maps 256 KiB chunks only after
releasing the heap guard; the boundary is published under that guard after
mapping succeeds. A rank-free, IRQ-enabled caller can grow synchronously on
allocation failure. BSP idle maintains mapped headroom when free space falls
below 1 MiB so guarded callers usually stay on the allocation-only path.
The growth window remains in the P4 slot already shared by user address
spaces. See the [growth audit](audit-stage6-heap-growth-2026-09-17.md).
The rank-40 physical-frame allocator stores one allocation bitmap in reserved
pages at the start of each usable RAM range. Returned frames beyond its 4,096
entry hot cache remain reusable through a bitmap scan; the bit also rejects
duplicate returns. Bitmap pages are excluded from allocatable-frame counts.
The heap now stores each live allocation's span in a header before its
payload, and stores free-list links inside freed spans. Its rank-50 guard
serializes those metadata writes; no fixed live-object or free-interval table
remains. The current mapped-byte ceiling and linear free-list search remain
separate limits.
The swap-state guard is rank 40 and releases before ATA transfers. Device,
keyboard decoder, journal-intent, mount-record, and key-vault metadata guards
are rank 10; none performs a blocking transfer while held. Keyboard captures
the caller's interrupt state before taking its guard to distinguish normal
IRQ-driven input from early-boot legacy polling.

The clean FAT32 page cache uses a rank-20 IRQ mutex for resident-page lookup,
copying, publication, and invalidation. A miss loads the 4 KiB page after
dropping that guard. Invalidation advances an epoch; a load that spans an
invalidation returns an I/O error instead of republishing stale cache data.

The shell's current-directory state is a short rank-10 IRQ-mutex section. It
copies the path into a fixed buffer before allocation or console output. The
global framebuffer terminal instead uses a rank-10 preemption mutex: it
prevents a local task switch while held but leaves device interrupts live
during long scroll and clear operations. Its rank-tracker updates briefly
mask local interrupts so an IRQ cannot observe a partial held-lock stack.
The terminal is never called from an IRQ handler and must not block or yield
while its rendering guard is held. Syscall writes use `file_fault_io` to
restore live interrupts around rendering even when syscall entry had IF=0.

This lock split covers the VFS/ATA slow-I/O boundary. FAT32 mutation still
crosses the storage and VFS layers without one transaction, so unrestricted
concurrent filesystem operations remain a separate production requirement.

## Stage 6 CPU lifecycle

Logical CPU slots and APIC identities remain stable while the online mask may
have holes. `online_count` is a population count, not an upper bound on valid
CPU indices. Scheduler placement uses a separate schedulable mask that excludes
APs draining or rebuilding their idle task; TLB shootdown snapshots the
physical online mask under the same transition lock that publishes an AP's
offline/online state. An offline request first plans all Ready-task moves under
the scheduler lock and commits them only if every task has a legal destination.
The CPU-owned idle checkpoint revalidates that plan before parking. A parked
AP retains its bootstrap stack and rejoins its original logical slot.
The requester cancels only a request the AP has not claimed; claimed offline
and online transitions use separate states and must reach a terminal result.
A rejected AP transition is rearmed for retry after AP cleanup. If a claimed
transition never completes, further hotplug requests remain disabled rather
than proceeding with uncertain CPU ownership.
The 4-CPU QEMU gate offlines CPU 1 while CPU 3 remains active, runs a worker on
CPU 3, and performs an acknowledged shootdown across the resulting hole.

## Stage 10.7 objects and data flow

`async_op::Handle` identifies a slot and a 32-bit generation. Its raw ABI uses
bits 0..15 for the slot, bits 32..63 for generation, and requires reserved bits
16..31 to be zero. A `Completion` is 16 bytes: signed status, reserved word and
64-bit value. The operation table records `TaskId` ownership, class, state and a
wait registration. Today ownership is task-bound; a shared per-process completion
port is Stage 10.8 work, not an existing property.

`network::UserSocket` associates a process owner and descriptor slot with a
smoltcp socket, peer, generation, async reference count and closing flag.
`SocketToken` carries slot, generation and owner for in-flight kernel operations.
Closing a pinned descriptor hides it from further descriptor operations; the
socket remains available to its existing tokens until the last pin is released.

`async_network::Request` stores owner, request identity, socket token, operation,
endpoint, length, kernel-owned bytes, result and generic completion handle.
The bounded queue holds pending, in-progress and completed requests. The worker
copies one request under the queue lock and releases that lock before entering
the socket runtime. Cancellation/teardown detach an in-progress request rather
than freeing its socket under the executing worker. The worker subsequently
releases that pin on completion or retry.

The submission path is:

1. `syscall.rs` checks WovenGuard network-device authority and validates arguments.
2. Send copies bytes from userspace immediately; receive retains only capacity.
3. `async_network.rs` pins the socket and allocates an owner-bound Network handle.
4. Queue publication precedes signaling the worker's latched scheduler event.
5. The worker attempts each queue slot at most once per pass. `WouldBlock` leaves
   that request pending and permits later requests to run. After the bounded
   pass, the worker waits for an event.
6. `network::poll()` drives smoltcp, releases its runtime lock, then notifies the
   worker if one locked queue snapshot sees pending or in-progress work.
7. Completion publication wakes a registered owner. Poll/wait copy completion
   and receive bytes to validated userspace destinations before consuming the
   request, releasing its socket pin and releasing the generic handle.

Failed copyout preserves the kernel result for retry. No asynchronous request
retains a userspace pointer. Cancellation does not roll back bytes already
accepted by the socket transport.

## Existing syscall boundary

| Number | API | Behavior |
| --- | --- | --- |
| 67 | Async cancellation | Owner-authorized cancellation; class-specific request cleanup |
| 77 | `AsyncNetSend` | Socket descriptor, input pointer and length; returns a completion handle |
| 78 | `AsyncNetRecv` | Socket descriptor and capacity; returns a completion handle |
| 79 | `AsyncNetPoll` | Nonblocking completion/data collection |
| 80 | `AsyncNetWait` | Event-driven wait followed by retry-safe collection |
| 81 | `AsyncTcpConnect` | Socket descriptor and packed endpoint; completes on connection readiness |

The audit corrections add no syscall numbers or capabilities. Existing network
policy gates remain at submission and collection; ownership checks remain on
handles. Teardown does not depend on retaining network permission.

## Concurrency and lock order

`irq_lock::IrqMutex` disables local maskable interrupts before taking its spin
mutex. Its guard releases the mutex before restoring the original interrupt
state and cannot be transferred to another thread. Nested guards preserve IF=0.
This addresses same-CPU preemption deadlock, which a plain spin mutex cannot
prevent even when the workload runs on only one CPU.

`irq_lock::PreemptMutex` is reserved for CPU-only sections that need live
interrupts. It increments a per-CPU no-preempt depth before acquiring the
spin mutex, and its guard releases the mutex and rank token before decrementing
that depth. The timer preemption path checks both this depth and the separate
I/O depth. It cannot be used in an IRQ handler or around a voluntary switch.

The network request queue, worker identity, generic completion table, socket
runtime and VirtIO-net transport use this guard. The runtime is rank 20 and
the transport is rank 30, matching runtime-to-transport packet flow. No task
may sleep or switch while holding it. The worker drops queue/runtime guards
before generic completion
publication or scheduler event waiting. Teardown can follow scheduler -> request
queue -> socket runtime. Network polling drops runtime before querying the queue;
transport polling releases its lock before socket-runtime acquisition. Runtime
packet handling can take runtime -> transport.

The socket worker remains CPU0-pinned. These changes do not introduce parallel
smoltcp workers, per-CPU network queues or unrestricted multicore I/O services.
The existing network runtime remains globally serialized. The guard does not
prove every cross-subsystem lock order; broader path audits remain necessary.

## Tests and evidence

`tests/async_network.rs` includes the production worker with deterministic host
socket/scheduler doubles. It covers blocked-before-ready requests, bounded passes,
parking, in-progress readiness notification, cancellation and racing owner cleanup.
`tests/irq_lock.rs` includes the production guard with host interrupt/mutex doubles.
`tests/test_tcp_harness.py` verifies host-server success and failure reporting.

The Ring-3 TCP probe queues receive before send on its second connection, forcing
the worker to handle a blocked operation without starving the send that enables
the host reply. It also checks invalid completion/data destinations and retries,
close while pinned, EOF, cancellation and owner teardown. The host requires both
TCP exchanges, required serial markers and QEMU debug-exit status 33.

`RUN-STAGE10.7.ps1` captures the chained acceptance transcript and snapshots QEMU
logs under `audit-artifacts/`. Stage 7.6 stress logs are retained per invocation.
`build.rs` tracks the actual kernel artifact so writing a log does not itself
trigger image reconstruction. Kernel changes still invalidate the image.

## Boundaries before later stages

Network progress still depends on the existing polling driver, not a new hardware
interrupt-driven NIC architecture. Send completion means local socket acceptance,
not remote acknowledgement; closing does not promise graceful draining. Public
socket descriptors remain process-local integer slots, while in-flight tokens
carry generations. Generation counters remain finite. Generic completions have
single-task ownership. These are
explicit architectural boundaries, not claims of a finished production network stack.

## Stage 10.8 completion ports

`completion_queue.rs` is the allocation-free reservation/FIFO core shared with
host tests. `completion_port.rs` adds process ownership, quotas, generation-tagged
handles, waiter registration and retry-safe batch claims. `async_op.rs` publishes
completion or cancellation into an associated port. Association and completion
serialize on the operation table before locking a port; scheduler notification
occurs after both are unlocked.

Syscalls 82 through 86 create, close, associate, poll and wait. See the
[ABI contract](completion-port-abi.md) for record layout, timeout and capacity
semantics. Port dequeue does not consume the original operation's data result.
`task::wait_for_event_until` extends the existing scheduler event latch with an
absolute deadline and timer-driven wakeup. Normal and forced process exit reclaim
ports after detaching owned operations.

`tests/async_runtime.rs` exercises production operation and port state with host
scheduler/lock doubles. `stage10_8.S` tests the real Ring-3 ABI;
`async_acceptance.rs` exercises multicore producers and a waiting consumer.
`RUN-STAGE10.8.ps1` preserves Stage 10.7 and adds 1/2/4-CPU completion-port boots.
Timers, event objects, IPC completion integration and per-thread cleanup remain
later roadmap work.

## Stage 10.9 timers and events

`async_events.rs` owns bounded generation-tagged timer and manual-reset event
slots. Timer deadlines use the monotonic tick source and are pumped by an
event-driven worker. Syscalls 87–94 expose create, wait, set, close and
sleep-until operations; timer and event waits produce the same generic async
completion records as other operations. Owner teardown cancels outstanding
waiters and releases every slot.

## Stage 13.10 Bluetooth architecture

Roadmap Stage 13.10 owns Bluetooth. Historical source/audit labels 13.10A–AC associated with AX200 are retained as historical Wi-Fi substage names under roadmap Stage 13.9 and must not be interpreted as Bluetooth milestones.

Stage 13.10A introduces `bluetooth_hci.rs` as a hardware-independent HCI protocol boundary. It owns bounded command packet construction, Command Complete parsing, command-credit state, opcode matching, and controller-status validation. The module is feature-gated by `stage13-10-test` and deliberately does not depend on xHCI, allowing the HCI state machine to be validated without pretending a physical transport exists.

The next layer, Stage 13.10B, should place a USB Bluetooth HCI transport beneath this protocol boundary using the existing xHCI core. Transport ownership must keep USB control transfers for HCI commands distinct from interrupt-IN HCI events and bulk ACL traffic. Hardware discovery, endpoint ownership, teardown, and DMA lifetime belong to the transport layer rather than the protocol parser. Later Bluetooth discovery, L2CAP, pairing/security, ATT/GATT and physical qualification remain above or beyond that boundary.


### Stage 13.10B USB HCI transport contract

Stage 13.10B extends the xHCI layer with Bluetooth USB descriptor semantics while preserving `bluetooth_hci.rs` as the transport-independent protocol boundary. The transport recognizes interface class/subclass/protocol E0/01/01 and requires three endpoint roles before accepting an interface: interrupt-IN for HCI events, bulk-IN for controller-to-host ACL data, and bulk-OUT for host-to-controller ACL data. HCI commands use the Bluetooth USB class control-request setup contract on endpoint zero.

The accepted 13.10B boundary parses and validates those roles and the command setup contract; it does not yet allocate dedicated Bluetooth transfer rings or claim live controller traffic. The next transport layer must own endpoint contexts, rings, DMA buffers, completion routing, bounded transfer sizes, teardown, and recovery. HCI event bytes should then be delivered upward to the protocol parser rather than interpreted inside xHCI.


### Stage 13.10C HCI transaction and transfer-execution boundary

`HciTransaction` is deliberately non-copyable mutable controller state. It owns the one-command-at-a-time credit/opcode contract: command bytes are encoded only after a credit is consumed, a second command is rejected while the transaction is outstanding, and readiness returns only after a matching successful Command Complete event.

The xHCI Bluetooth boundary maps the Stage 13.10B descriptor result into bounded transfer plans for endpoint-zero HCI commands, interrupt-IN HCI events, and bulk ACL input/output. These plans describe ownership and size constraints; they do not themselves claim that a USB transfer was executed. The next implementation slice must materialize the plans as dedicated xHCI endpoint contexts, transfer rings, DMA buffers and completion routing, then feed received HCI event bytes back into `HciTransaction`.


### Stage 13.10D live USB HCI mechanics and initialization

Stage 13.10D turns the accepted transfer descriptions into xHCI-owned execution state. The controller owns separate Bluetooth event, ACL-IN and ACL-OUT rings and DMA pages rather than reusing HID ownership. USB endpoint addresses are converted to direction-aware xHCI DCIs, and endpoint contexts distinguish interrupt-IN, bulk-IN and bulk-OUT endpoint types. HCI commands use an endpoint-zero control-OUT-with-data path; HCI events use interrupt-IN; ACL traffic uses the dedicated bulk rings.

The protocol layer remains independent of USB. `ControllerInitializer` composes `HciTransaction` into a bounded startup sequence: Reset is issued first, its matching successful Command Complete advances to Read Local Version, and only the matching successful version completion advances the controller to Ready. Transport bytes therefore cross into HCI state only at the protocol boundary; xHCI does not interpret controller policy.

The Stage 13.10D CI acceptance is deterministic and does not imply physical-radio qualification. A real Bluetooth USB controller must still validate endpoint behavior, timing, teardown/recovery and hardware-specific interoperability. Higher layers should build discovery/scanning and capability parsing above this accepted initialization boundary rather than adding policy to xHCI.


### Stage 13.10E discovery and controller capabilities

Stage 13.10E keeps discovery policy in the hardware-independent HCI layer. `DiscoveryState` owns a bounded 16-device BR/EDR Inquiry result set, deduplicated by Bluetooth device address. Inquiry event parsing validates the HCI parameter length before reading response records and stores only the fields needed by later link establishment: address, page-scan repetition mode, class-of-device and clock offset. xHCI remains responsible only for moving event bytes.

Controller capability parsing follows the same boundary. Read Local Version, Read BD_ADDR and Read Local Supported Commands Command Complete events are checked for the expected opcode, successful controller status and complete return payload before typed data is exposed. This prevents later connection/security layers from inferring controller support from transport presence alone.

The accepted 13.10E boundary is classic BR/EDR discovery plus local-controller capability discovery. It deliberately does not fold BLE scanning, connection establishment, pairing, L2CAP, ATT/GATT or policy into the transport. The next link layer should consume `DiscoveredDevice` records and capability data while retaining bounded handle/state ownership.


### Stage 13.10F ACL connection and ownership lifecycle

The Bluetooth HCI layer now owns the mapping from a discovered BR/EDR peer to a live ACL controller handle. `LinkState` is bounded to eight links and treats the controller handle plus Bluetooth address as an ownership pair. Replayed identical Connection Complete events are harmless, while a handle reused for another address or an address appearing under another live handle is rejected rather than silently aliasing authority.

`AclPacket` owns HCI ACL framing independently of xHCI. It masks the 12-bit connection handle, carries packet-boundary and broadcast flags, applies a bounded 1024-byte software payload ceiling, and validates declared lengths before exposing payload bytes. `LinkState::outbound_acl` and `inbound_acl` require the handle to remain live; Disconnection Complete removes the link before further ACL traffic can be accepted.

This boundary is intentionally below L2CAP. Stage 13.10G should parse/build the L2CAP basic header and add bounded channel identifiers/state while continuing to use the 13.10F ACL handle as the lower-layer authority anchor.


### Stage 13.10G L2CAP framing and channel ownership

L2CAP is layered above the 13.10F ACL authority boundary rather than owning transport handles itself. `L2capFrame` validates the Basic Mode length/CID header and uses fixed-capacity payload storage. `L2capChannels` owns at most eight established dynamic channels and records the ACL handle, PSM, local CID and remote CID for each.

Signaling establishes and tears down channel authority. Data transmission is permitted only through an established channel and is encoded for its remote CID; receive dispatch requires the same live ACL handle plus the channel's local CID. Removing the channel immediately makes both transmit and receive paths fail closed. This keeps higher Bluetooth protocols from treating CIDs as ambient/global authority.

Stage 13.10H should consume these owned channels when adding authentication/pairing and encryption state. It must not weaken the lower-layer ACL/CID ownership checks.


### Stage 13.10H Bluetooth trust authority

Bluetooth security is modeled as authority derived from controller-confirmed state, not inferred from connection existence. `LinkSecurityState` tracks authentication/encryption progression per owned ACL handle. `LinkKeyStore` owns a bounded peer-address-to-link-key association and answers controller Link Key Requests with a stored key or an explicit negative reply.

Secure Simple Pairing events are parsed into policy decisions: IO capability can be declared, but numeric confirmation and passkey acceptance require an explicit caller decision. The kernel does not silently accept a pairing prompt.

The strongest software trust predicate binds four facts: the ACL handle is still live, its peer address owns a stored link key, authentication completed successfully, and encryption was confirmed enabled by the controller. Key removal or ACL teardown therefore revokes trusted authority without relying on stale security state.


### Stage 13.10I Bluetooth LE discovery and link authority

Bluetooth LE is modeled with state distinct from the BR/EDR discovery and link-key trust paths. `LeDiscoveryState` owns a fixed-capacity table keyed by address type plus device address, because an LE peer identity cannot safely be reduced to the six address bytes alone. Advertising reports are accepted only when the LE Meta Event framing, subevent, declared lengths, address type and legacy advertising-data bounds are internally consistent.

`LeLinkState` is the LE controller-handle authority boundary. LE Create Connection consumes an owned discovery record, carrying its peer address type and address into the controller command. A successful LE Connection Complete establishes a bounded handle/role/address/connection-parameter record. Conflicting handle or peer ownership is rejected rather than aliased. Disconnection Complete removes the record and makes the handle stale immediately.

This layer intentionally does not reuse BR/EDR `LinkKeyStore` as BLE bonding state. Stage 13.10J ATT/GATT should consume live `LeLinkState` ownership and add bounded ATT transaction and attribute/service state. BLE SMP, long-term keys, privacy and bonding require a separate security model in a later slice.


### Stage 13.10J Bluetooth ATT/GATT authority and service model

ATT/GATT consumes `LeLinkState` as its lower-layer authority rather than treating an ATT attribute handle as global authority. `AttDatabase` is fixed-capacity, uses nonzero handles and bounded values, and rechecks the live LE controller handle on each transaction or discovery operation. Read/write permission checks remain in ATT so a higher GATT abstraction cannot bypass them.

`GattDatabase` maps Primary Service declarations and Characteristic Declaration/value pairs into the ATT attribute space. Characteristic declarations carry a value handle and bounded properties; characteristic values retain their own ATT permissions. Service and characteristic discovery are handle-range bounded and remain tied to the requesting live LE link.

`GattSubscriptions` records notification/indication authority as the tuple of LE connection handle and characteristic value handle. CCCD-style enable/disable state is bounded, invalid bit combinations are rejected, and every emitted Handle Value Notification or Indication rechecks both subscription mode and live LE ownership. Disconnect therefore makes retained subscription state unusable even before later cleanup/hardening.

This stage is a deterministic kernel protocol foundation, not a claim of complete physical BLE GATT interoperability. Stage 13.10K should build standard/custom BLE profile services on this authority boundary. BLE SMP, bonding/LTK state and persistent key protection remain separate security work and must not reuse BR/EDR link-key authority.


### Stage 13.10K BLE profile/service layer

The profile layer composes services from Stage 13.10J GATT primitives instead of bypassing ATT. Device Information and Battery Service attributes therefore inherit live `LeLinkState` checks and ATT read/write permissions. Battery level is bounded to 0..=100 and its notification path uses `GattSubscriptions` scoped to the exact LE handle and Battery Level value handle; state is committed only after notification authorization succeeds.

The WovenHat OS BLE service demonstrates the custom-service boundary with separate status and command characteristics. Status is readable but not writable; command is writable but not readable. A disconnected LE peer cannot continue using either path because ATT transactions recheck link ownership.

GATT characteristic creation preflights capacity for both declaration and value attributes before mutation. This preserves database consistency under fixed-capacity exhaustion and prevents partially-created characteristic state.

Stage 13.10L should harden controller/link/service lifecycle, teardown, malformed-event handling and recovery while retaining these ownership and permission boundaries.


### Stage 13.10L Bluetooth lifecycle authority

`BluetoothLeLifecycle` is the coordination boundary for LE link ownership and GATT subscription authority. Teardown is ordered: the live link is removed only after a structurally valid successful Disconnection Complete event identifies an owned handle; subscriptions for that exact handle are then revoked. Failed parsing or stale ownership returns an error before subscription or generation mutation.

Controller reset is a stronger invalidation boundary and clears all live LE links and all GATT subscriptions. The lifecycle generation advances on each successful disconnect or reset, providing a bounded epoch signal for later controller/service recovery work.

Reconnect deliberately creates no implicit subscription continuity. A re-established link must explicitly configure its GATT subscription again. This prevents stale CCCD-style authority from crossing disconnect/reset generations.

The Stage 13.10L stress contract repeats these invariants across 32 cycles and alternates disconnect and reset paths. It is a software-state hardening boundary, not evidence of physical radio/controller recovery or interoperability.


### Stage 13.10R Bluetooth secure-session identity authority

`LeSecuritySessions` stores the LE peer address type and address alongside the
controller handle. `session_for_link` is the security boundary used by both
ATT/GATT transactions and secured notification/indication emission: a session
is valid only while the live `LeLinkState` entry has the same handle and peer
identity. Handle reuse therefore cannot inherit the previous peer's protected
authority even if transient teardown cleanup was missed.

This remains bounded deterministic software state. It does not claim physical
BLE controller recovery, SMP/LTK provisioning, radio interoperability or
hardware qualification.


### Stage 13.10S BLE minimum key-size authority

`AttSecurityPolicy` carries an optional minimum encryption key size in addition
to its authentication requirement. `AttDatabase::transact_secured` and
`GattSubscriptions::emit_secured` both reject a live authenticated session
whose negotiated key is below that bound. The policy is checked after the
identity-bound session lookup and before either an ATT response or an outbound
notification/indication is produced, keeping inbound and outbound authority
consistent.


### Stage 13.10T BLE outbound authorization acceptance

The Stage 13.10 runtime gate requires a dedicated outbound key-size marker in
addition to the inbound ATT/GATT checks. Its self-test exercises the full
subscription path: weak authenticated sessions are denied before packet
encoding, and a strong identity-bound session produces the expected
notification.


### Stage 13.11A BLE SMP foundation

`BleSmpFixedChannel` is the bounded protocol boundary above a live
`LeLinkState` and L2CAP fixed CID `0x0006`. It parses and validates pairing
parameters before advancing `SmpPairingState`; malformed fields, unsupported
authentication/key-distribution bits, stale handles and non-SMP CIDs fail
closed. This layer does not claim cryptographic derivation, user interaction,
bond persistence or physical-controller qualification.


### Stage 13.11B BLE SMP LTK distribution

The SMP distribution boundary accepts legacy Encryption Information and Master
Identification only for the live negotiated pairing handle. It validates key
length, identity and negotiated-size consistency before storing material in
`LeBondStore`, preserving the existing address/type ownership model and
fail-closed teardown semantics.

Pending distribution retains the peer address identity captured with the
Encryption Information PDU. Master Identification is accepted only while the
same identity remains live, preventing handle reuse from importing stale LTK
material into another peer's bond.


### Stage 13.11C BLE SMP confirm/random verification

`SmpConfirmState` binds Pairing Confirm and Pairing Random processing to the
negotiated `SmpPairingState`, live handle and address-aware confirm inputs. The
`c1` and `s1` boundaries are explicit and bounded; a confirm mismatch enters
the failed state for that pairing attempt. This does not claim complete
production pairing UX or key-derivation integration.


### Stage 13.11F BLE controller encryption authority

`SmpConfirmState::start_encryption_command` owns the bounded LE Start
Encryption command layout and returns a pending authority record containing the
live peer identity, negotiated key size and authentication result.
`LeSecuritySessions::encryption_change_from_pairing` accepts only a matching
controller completion event and live link; failure, disabled state, stale
handles or identity mismatch revoke authority.


### Stage 13.11G BLE identity/privacy foundation

The bounded identity store accepts SMP Identity Information and Identity Address
Information only for the live pairing handle. IRK-backed private-address
resolution is deterministic and fail-closed, with duplicate identity replacement
remaining bounded. This identity layer complements, but does not replace, the
peer-bound LTK distribution and controller encryption authority checks.


### Stage 13.11H BLE SMP lifecycle and authentication closure

`SmpLifecycle` is the bounded orchestration boundary for legacy BLE SMP state.
It serializes Pairing Request/Response and key/identity distribution against one
live LE handle, rejects duplicate, out-of-order and wrong-handle traffic, and
clears transient distribution state on Pairing Failed, disconnect and reset.
Handle reuse therefore cannot silently inherit the previous pairing lifecycle.

Authentication authority is deliberately separated from association-model
negotiation. `SmpPairingState::authentication` selects Just Works, Passkey
Entry or OOB policy, but feature exchange alone does not mint persistent
authenticated authority. `SmpAuthenticationProof` supplies the method-specific
temporary key boundary: Just Works uses the zero TK and remains unauthenticated,
Passkey Entry accepts only 0..=999999 and encodes that value into the legacy TK,
and OOB requires an explicit 128-bit TK. Only
`SmpConfirmState::verify_random_with_proof` may promote a verified Passkey/OOB
attempt to authenticated session authority after Confirm/Random succeeds.

The compatibility raw-TK `verify_random` path may prove confirm consistency but
cannot mint MITM-authenticated authority. Likewise, legacy key distribution no
longer marks a persistent bond authenticated merely from negotiated pairing
capabilities. The Stage 13.11 runtime harness requires the G identity/privacy
marker, H lifecycle marker, and the persistent-authentication audit marker on
the 1/2/4-core acceptance matrix.

This closes the deterministic software authority model for Stage 13.11. It does
not claim Bluetooth qualification, physical-radio interoperability, side-channel
resistance, formal cryptographic verification, or an external security audit.
Those require separate hardware/specification validation before production
security claims.


## Stage 14.1 WovenNet IPv4 core closure (2026-10-06)

Stage 14.1 is the accepted IPv4 production-networking foundation above the
VirtIO-net/smoltcp runtime. The live QEMU/slirp gate exercises DHCP lease
acquisition, DNS A-record resolution, ICMP reachability, host-to-guest UDP
traffic and a bidirectional TCP round trip on the same runtime used by normal
boots. The release workflow runs this contract on the 1/2/4-core matrix with
the `stage14-1-test` feature and requires the aggregate
`[S14.1] WovenNet IPv4 core: PASSED` marker.

DHCP transition authority is centralized in `apply_dhcp_locked` and
`apply_static_locked`. A resolver change cancels every outstanding bounded
DNS query before the DNS server is replaced, so a response issued under an old
resolver cannot be mistaken for work belonging to the new lease. The
`[S14.1D]` acceptance test drives DHCP-to-lease and lease-to-static recovery
and verifies address, prefix, gateway, resolver and query cleanup.

DNS query IDs are generation-tagged capabilities rather than reusable raw slot
numbers. `dns_start` records a non-zero generation with the smoltcp query
handle; `dns_poll` and `dns_cancel` require the same slot/generation pair.
After cancellation or slot reuse, an old token therefore fails with
`SocketError::Invalid` and cannot poll or cancel the replacement query. This
is covered by the `[S14.1G]` and `[S14.1H]` lifecycle gates.

Userspace sockets remain bounded to `MAX_USER_SOCKETS = 16`. Ownership is
checked on descriptor operations, while asynchronous operations use
`SocketToken { slot, generation, owner }` so stale pinned authority cannot
cross descriptor reuse. Close revokes descriptor access immediately; UDP can
retire immediately when unpinned, while TCP retains transport state only for a
bounded graceful-drain interval. The live network test now explicitly retires
its TCP owner and waits for `user_sockets == 0` before the resource-stress
gate begins, preventing acceptance-test resources from leaking into the next
authority domain.

The `[S14.1S]` stress gate fills all 16 userspace socket slots, requires the
17th open to return `SocketError::NoSlot`, retires the full set, fills the
table again with unique in-range descriptors, verifies exhaustion again, and
requires the table to return to zero. It also verifies dynamic/private
ephemeral allocator wraparound across `65533 -> 65534 -> 49152 -> 49153`.
Failure checkpoints remain in the test so future regressions identify the
broken phase rather than collapsing into a generic marker failure.

Stage 14.1 closure was accepted by GitHub Actions release-validation run #1199
(run 37337619836) at commit
`c1cce4da89ab7527290b1616207139a1b038b05a`. The full release-validation job
was green, including the live Stage 14.1 1/2/4-core acceptance matrix and all
earlier mandatory release gates.

### Stage 14.1 boundary and follow-up hardening

This closure establishes the bounded IPv4 core and its current lifecycle
invariants; it is not a claim of a complete POSIX/BSD socket stack, IPv6,
production firewall/NAT policy, TLS, physical-NIC interoperability across
hardware families, or exhaustive network fault injection. The current
ephemeral-port stress proves range/wrap behavior and bounded socket recovery;
it does not yet exhaustively prove collision avoidance across every concurrent
TCP/UDP local-endpoint combination. Future networking work should add explicit
live-endpoint collision selection/retry semantics before making that stronger
claim.

### Stage 14.2 IPv6 closure

Stage 14.2 is accepted for the bounded WovenNet IPv6 scope defined by the
master roadmap: IPv6, DHCPv6, and IPv6 Neighbor Discovery.

The implementation includes bounded IPv6 address/prefix types and
solicited-node multicast derivation; ICMPv6 Neighbor Discovery parsing for
RS/RA/NS/NA; IPv6 pseudo-header checksum generation/verification; router and
prefix lifetime state; neighbor reachability and Duplicate Address Detection;
SLAAC address lifecycle; DHCPv6 message parsing/serialization, client
Solicit/Advertise/Request/Reply state, and IA_NA/IAADDR lease lifetimes; an
interface-level ICMPv6 ingress state boundary; and runtime installation of a
MAC-derived IPv6 link-local /64 alongside the existing IPv4 address.

The closure hardening gate rejects NDP traffic whose IPv6 Hop Limit is not
255. It also requires link-local Router Advertisement sources, rejects
Neighbor Advertisements from the unspecified source, and constrains DAD
Neighbor Solicitations from the unspecified source to the tentative address's
solicited-node multicast destination. Existing ICMPv6 checksum validation
remains mandatory before state mutation.

The accepted serial closure markers include
`[S14.2J] WovenNet IPv6 interface ingress: PASSED`,
`[S14.2K] WovenNet runtime IPv6 integration: PASSED`, and
`[S14.2L] WovenNet NDP ingress hardening: PASSED`. The network QEMU harness
carries these requirements forward into the Stage 14.3 and Stage 14.4 feature
gates so later networking work cannot silently regress the accepted IPv6
boundary.

This acceptance is intentionally scoped. It does not claim physical-NIC IPv6
interoperability across hardware families, a complete POSIX/BSD IPv6 socket
surface, production route ownership/policy, or exhaustive RFC 4861/8415
interoperability. Those concerns remain appropriate follow-up hardening or
belong to Stage 14.3+.

### Stage 14.3 socket API closure

Stage 14.3 is accepted for the bounded WovenNet Socket API scope.

The existing owner-scoped socket table remains the authority root for UDP and
TCP open, bind/listen, connect, send, receive, peer inspection, close, and
process cleanup. Descriptors are checked against their owning process, while
asynchronous work uses `SocketToken` values containing the slot, owner, and
generation. Closing a descriptor revokes descriptor authority immediately;
after outstanding pins are released and the slot is reused, an old token
cannot operate on the new socket generation.

The endpoint boundary preserves the legacy packed IPv4 representation for
compatibility and adds `SocketEndpointV1`, a fixed-layout versioned endpoint
representation for IPv4 and IPv6. It rejects zero ports, malformed IPv4
encodings, unspecified or multicast destinations, unknown address families,
and non-zero reserved ABI fields. The versioned representation is wired into
real socket connect and peer inspection operations rather than existing only
as a serialization helper.

The accepted serial gates are
`[S14.3] WovenNet socket API boundary: PASSED` and
`[S14.3A] WovenNet socket authority lifecycle: PASSED`. The QEMU networking
harness requires both markers for Stage 14.3 and carries them forward into
Stage 14.4, preventing routing work from silently weakening socket authority.

This acceptance does not claim a complete POSIX/BSD socket surface,
exhaustive physical-NIC IPv6 interoperability, or production routing-policy
ownership. Those remain later platform hardening or Stage 14.4+ concerns.

### Stage 14.4 routing-table closure

Stage 14.4 is accepted for the bounded WovenNet routing-policy scope.

`WovenRouteTable` is a fixed eight-slot, generation-tagged, owner-scoped
dual-stack policy table. IPv4 prefixes from /0 through /32 and IPv6 prefixes
from /0 through /128 are normalized before storage. Route lookup rejects
address-family mismatches and selects the longest matching prefix, then the
lowest metric and oldest generation as deterministic tie-breakers. Invalid
prefixes, unusable gateways, and mixed network/gateway address families are
rejected at insertion.

Each `WovenRoute` records its owner and generation. Exact route handles are
required for removal, cross-owner removal returns `WrongOwner`, and
`remove_owner_routes` provides bounded deterministic teardown when an
authority domain exits. This gives later WovenGuard and Network Manager work
an explicit routing authority boundary instead of a globally mutable table.

The policy table is now connected to the real smoltcp route set through
controlled `install_woven_route` and `remove_woven_route` boundaries.
Installation is idempotent for an identical CIDR/gateway pair and reports
bounded-capacity failure rather than silently dropping a route. Removal is
exact and a stale second removal is rejected. The live acceptance gate proves
this lifecycle for both an IPv4 route and an IPv6 /64 route without replacing
the existing DHCP/static default route.

The accepted serial gates are
`[S14.4] WovenNet routing table: PASSED` and
`[S14.4A] WovenNet live route integration: PASSED`. The Stage 14.4 QEMU
harness requires the live marker in addition to the inherited Stage 14.2 and
14.3 networking gates.

This acceptance does not claim a userspace route-management service, dynamic
routing protocols, multi-interface policy routing, or exhaustive physical-NIC
interoperability. Those remain appropriate Network Manager and platform
hardening work. Stage 14.5 can now build WovenGuard firewall policy on the
accepted socket, IPv6, and routing authority foundations.

### Stage 14.5A WovenGuard firewall policy foundation closure

Stage 14.5A is accepted for the bounded policy foundation scope.

`FirewallPolicy` stores at most 32 rules and fails closed on capacity,
duplicate identifiers, invalid IPv4/IPv6 prefix lengths, and reversed port
ranges. Rules match inbound, outbound, or forward traffic by protocol,
optional source/destination CIDR, and optional source/destination port range.
Evaluation is deterministic: the lowest numeric priority wins and the rule ID
is the stable tie-breaker. Traffic with no matching rule receives the
configured default action.

IPv4 and IPv6 prefix matching covers /0 through /32 and /0 through /128,
including non-octet prefix lengths. Global policy access is serialized through
the IRQ-aware mutex boundary so later live enforcement does not introduce an
unsynchronized mutable policy table.

The accepted serial gate is
`[S14.5A] firewall policy foundation PASSED`. The Stage 14.5 feature inherits
the accepted Stage 14.1-14.4 networking gates and is exercised by the QEMU
network harness on 1, 2, and 4 CPUs. Acceptance was recorded by GitHub Actions
run #1267 on commit `f8b79ac7`.

This closure deliberately does not claim live filtering. Stage 14.5B must
connect policy evaluation to real ingress and egress packet paths while
preserving DHCP, NDP/RA/SLAAC, socket authority, and routing behavior. A
forwarding hook should be added only where an actual forwarding path exists.


### Stage 14.5B WovenGuard live packet enforcement closure

Stage 14.5B is accepted for the live virtio ingress/egress enforcement scope.
The firewall now has a packet-path bridge from Ethernet frames to the Stage
14.5A policy model. IPv4 and IPv6 headers are decoded into source/destination
addresses and TCP/UDP ports or ICMP/ICMPv6 protocol identity before policy
evaluation.

The enforcement points match the smoltcp device contract. Inbound denied
frames are discarded in `VirtioSmolDevice::receive` before an RX token is
exposed to smoltcp. Outbound frames are evaluated in `WovenTxToken::consume`
after smoltcp has constructed the Ethernet frame and before
`virtio_net::transmit` submits it. This avoids pretending that an
`RxToken::consume` callback can cancel delivery after a token has already
been returned.

Non-IP Layer-2 frames are preserved at this stage so ARP and other link-control
traffic are not accidentally removed by an IP firewall parser. IPv6
ICMPv6 policy metadata supports the NDP/RA/SLAAC control plane inherited from
Stage 14.2.

Live enforcement uses an explicit atomic activation state. The policy itself
retains default-deny semantics, but enforcement remains inactive during
bootstrap and becomes active only through the firewall lifecycle boundary.
This separation was required after the first Stage 14.5B gate correctly
exposed a DHCP bootstrap deadlock when default-deny enforcement was active
from boot.

Acceptance requires `[S14.5B] live packet enforcement PASSED` plus all
inherited WovenNet markers. GitHub Actions run #1277 on commit `1d1ce646`
passed build, Clippy with `-D warnings`, live DHCP/DNS/ICMP/UDP/TCP
regressions, and the dedicated Stage 14.5B QEMU matrix on 1, 2, and 4 CPUs.

This closure does not claim a userspace firewall administration API, persistent
rule configuration, logging/counters, stateful connection tracking, NAT, an
actual routed forwarding hook, or completed Wi-Fi enforcement parity. Those
remain later WovenGuard/network-management work.


### Stage 14.5C WovenGuard firewall policy authority closure

Stage 14.5C is accepted for privileged firewall policy management. The
capability model now distinguishes ordinary `NetworkIo` authority from the
new `NetworkAdmin` authority. `NetworkIo` remains available to normal
userspace for sockets; it is deliberately insufficient to mutate global
firewall state.

`NetworkAdmin` is included in the Kernel and SystemService domain ceilings
but excluded from the User and Restricted ceilings. Administrative wrappers
require that capability before rule add, remove, or replace operations,
default-action changes, or live-enforcement activation/deactivation reach the
global policy object. Unauthorized calls return an explicit authority error
without changing policy state.

The Stage 14.5C self-test proves both sides of the boundary: userspace
authority is rejected for policy mutation and enforcement activation, while
`NetworkAdmin` can perform the bounded mutation lifecycle. It also verifies
the WovenGuard domain ceiling itself so future capability refactors cannot
silently grant firewall administration to ordinary users.

Acceptance requires `[S14.5C] firewall policy authority PASSED` plus all
inherited WovenNet/WovenGuard network markers. GitHub Actions run #1287 on
commit `b81b3a76` passed build, Clippy with `-D warnings`, and the QEMU
1/2/4-core matrix.

This closure does not yet claim complete transport parity, malformed-IP
fail-closed parsing, IPv6 extension-header inspection, a live routed Forward
hook, persistent rules, logging/counters, stateful connection tracking, or
NAT. Those are subsequent firewall/network-management hardening work.
