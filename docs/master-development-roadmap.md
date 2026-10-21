WovenHat OS — Stage 10.7 → Version 1.0 Master Roadmap
Phase I — Finish the asynchronous kernel foundation
Stage 10.7 — Asynchronous TCP

Stage 24.59 - Deterministic Change-PIN acceptance
Accepted the complete desktop PIN-rotation path: one kernel-owned salt is
shared by the identity authority and durable account record, the old PIN is
rejected after relock, and the new PIN unlocks. Production-feature Clippy,
host tests, and QEMU 1/2/4-CPU acceptance passed. Account creation was
accepted in Stage 24.15; physical TPM/USB qualification remains an open
release prerequisite.

Stage 24.58 - Physical TPM broker boot activation
Integrated signed policy validation, attestation public-key discovery, fresh
TPM nonce generation, credential-root promotion, and broker installation into
boot. The desktop remains available when TPM activation fails closed; live
TPM qualification remains a release gate.

Stage 24.57 - Physical TPM credential-root read boundary
Implemented the bounded kernel path that validates the configured NV public
area, applies the PCR policy, performs authenticated policy-session NV Read,
refreshes the nonce, and flushes/wipes all transient state. Physical TPM
execution and hardware qualification remain release gates.

Stage 24.56 - Signed TPM broker record generator
Added the warning-clean `tools/tpm-broker-config` release tool. It validates
production bounds and emits a fixed signed `TCB1` policy record only with an
explicit matching TPM policy signing seed and trusted public key; no
development-key fallback or device I/O exists.

Stage 24.55 - Physical TPM readiness preflight
Added a read-only host preflight for TPM visibility/readiness, emulation,
signed broker-record presence, and removable-disk safety. The observed machine
has no accessible TPM or software TPM backend; physical qualification is not
claimed and no media was changed.

Stage 24.54 - Kernel-derived TPM authorization secret
Bound credential-root leases to the validated NV index and policy, rejected
metadata-changing rotations, and derived the attestation authorization value
inside kernel memory during broker installation. The secret is never persisted
or accepted from userspace.

Stage 24.53 - Signed TPM broker provisioning record
Implemented a fixed-size signed `TCB1` record for broker policy, bounded
validation of the reserved NV index, attestation handle, quote scheme, PCR,
expected digest, attestation Name, and NV policy, plus WovenFS persistence and
boot revalidation. Authorization secrets remain outside durable storage.

Stage 24.52 - Physical TPM credential broker boundary
Implemented the kernel-owned broker that validates release policy inputs,
executes one fresh TPM PolicyPCR/Quote sequence, verifies and flushes it, and
publishes the move-only identity decision. Syscalls 234/233 and libwoven
wrappers provide request/consume sequencing. Physical provisioning, live TPM
login, recovery, and qualification remain required.

Stage 24.51 - Identity hardware-decision consumption
Implemented owner/session/name-bound move-only `CredentialDecision` handoff
through syscall 233, with a bounded pending slot, one-shot generation check,
and audit recording. The physical TPM broker producer, physical credential
login, reboot recovery, and hardware qualification remain prerequisites.

Stage 24.50 - Hardware credential decision boundary

Implemented root-generation-bound TPM quote authorization: a bounded identity
context derives the expected nonce, and signed Name, ReadPublic, PCR, and
cryptographic signature checks must all pass before a generation-tagged
decision is returned. Host tests, Clippy, executable QEMU, and signed
1/2/4-CPU acceptance pass. Live physical TPM success, identity-session
consumption, recovery, qualification, and release readiness remain required.

Stage 24.49 - Credential-root lifecycle

Implemented a generation-tagged, non-copyable kernel credential-root lease
with validated installation, transactional rotation, bounded derivation, and
explicit zeroizing revocation. Host tests, Clippy, executable QEMU, and signed
1/2/4-CPU acceptance pass. Live TPM NV execution, credential decisions,
recovery, qualification, and release readiness remain required.

Stage 24.48 - TPM custody authorization boundary

Implemented a private-construction kernel custody token for TPM policy-session
contexts. Issuance requires a validated live CRB descriptor and an accepted
signed TAN1 policy on release boots; context persistence and recovery require
the token in addition to WovenFS storage authority. Host tests, Clippy,
executable QEMU, and signed 1/2/4-CPU acceptance pass. Physical TPM execution,
key lifecycle, credential decisions, recovery, qualification, and release
readiness remain required.

Stage 24.47 - TPM policy-session nonce refresh

Implemented strict TPM quote authorization-area decoding and managed lease
nonce refresh. Password-session responses may have an empty nonce; policy
session responses must return a bounded nonce, which is retained before the
one-shot lease becomes stale. Host tests, Clippy, executable QEMU, and signed
1/2/4-CPU acceptance pass. Physical TPM execution, durable custody
authorization, key lifecycle, credential decisions, recovery, qualification,
and release readiness remain required.

Stage 24.45 - Durable TPM context custody boundary

Implemented versioned dual-copy WovenFS custody for opaque TPM policy-session
contexts, with fixed bounds, SHA-256 integrity, sequence arbitration, and
fail-closed metadata validation. Host tests, Clippy, executable QEMU, and
signed 1/2/4-CPU acceptance pass. Boot integration, secure authorization,
nonce refresh, key lifecycle, credential decisions, recovery, qualification,
and release readiness remain required.

Stage 24.44 - TPM policy-session context save/load

Implemented bounded `TPM2_ContextSave`/`ContextLoad`, one-use snapshots,
lease invalidation after save, and fresh generations after load. Host tests,
Clippy, executable QEMU, and signed 1/2/4-CPU acceptance pass. Cross-reboot
custody, nonce refresh, key lifecycle, credential decisions, recovery,
qualification, and release readiness remain required.

Stage 24.43 - Managed TPM policy-session lifecycle

Implemented generation-tagged policy-session leases, stale/active guards,
one-shot quote consumption, and exact `TPM2_FlushContext` cleanup. Host
tests, Clippy, executable QEMU, and signed 1/2/4-CPU acceptance pass. Context
save/load, nonce refresh, key lifecycle, credential decisions, recovery,
qualification, and release readiness remain required.

Stage 24.42 - Policy-session quote HMAC derivation

Implemented SHA-256 Quote cpHash/HMAC derivation for null-salt/null-bind
policy sessions and integrated the derived HMAC with authenticated quote
execution. Host tests, Clippy, executable QEMU, and signed 1/2/4-CPU
acceptance pass. Session refresh, provisioned-key lifecycle, credential
decisions, recovery, qualification, and release readiness remain required.

Stage 24.41 - Live policy-session transcript execution

Implemented bounded CRB execution for `TPM2_StartAuthSession` and
`TPM2_PolicyPCR`, including returned session handle/nonce and exact response
validation. Host tests, Clippy, executable QEMU, and signed 1/2/4-CPU
acceptance pass. Session-state refresh, HMAC derivation, credential
decisions, recovery, qualification, and release readiness remain required.

Stage 24.40 - Policy-session TPM2 Quote framing

Extended bounded `TPM2_Quote` execution with explicit non-password policy
session authorization framing. Caller-provided session handle, nonce,
attributes, and HMAC are bounded and preserved; password-session downgrade is
rejected. Host tests, Clippy, executable QEMU, and signed 1/2/4-CPU
acceptance pass. Session creation, PolicyPCR execution, HMAC derivation,
credential decisions, recovery, qualification, and release readiness remain
required.

Stage 24.39 - Bounded TPM2 Quote execution

Implemented bounded sessions-tagged `TPM2_Quote` framing, CRB execution, and
strict quote/signature response parsing for SHA-256 PCR selection. The
result is evidence only and must pass the existing fail-closed authorization
boundary. Host tests, Clippy, executable QEMU, and signed 1/2/4-CPU
acceptance pass. Physical TPM execution, non-empty authorization sessions,
credential decisions, recovery, qualification, and release readiness remain
required.

Stage 24.38 - Boot-time TPM Name policy loading

Implemented the release system-apps boot hook that reads and revalidates the
durable signed TAN1 attestation Name policy after filesystem mount. Missing,
malformed, or tampered policy data fails closed by disabling hardware
credentials. Host tests, Clippy, executable QEMU, and signed 1/2/4-CPU
acceptance pass. Live TPM execution, credential decisions, recovery,
physical qualification, and release readiness remain required.

Stage 24.37 - Fail-closed TPM quote authorization integration

Implemented one structured authorization path binding signed TPM identity,
ReadPublic key, quote nonce/PCR evidence, and RSA/ECC signature verification.
The executable QEMU self-test, host tests, Clippy, and signed 1/2/4-CPU
acceptance pass. Boot loading, live TPM execution, credential decisions,
recovery, physical qualification, and release readiness remain required.

Stage 24.36 - Durable signed TPM Name policy boundary

Implemented journal-backed WovenFS persistence and revalidated reads for the
signed TAN1 TPM Name policy. The executable QEMU self-test, host tests,
Clippy, and signed 1/2/4-CPU acceptance pass. Physical provisioning, boot
loading, live TPM trust, quote authorization, recovery, qualification, and
release readiness remain required.

Stage 24.35 - Signed TPM attestation-Name policy

Implemented a bounded TAN1 Ed25519-signed expected TPM Name policy feeding
the Name-binding boundary. The executable QEMU self-test, host tests, Clippy,
and signed 1/2/4-CPU acceptance pass. Durable storage, live TPM retrieval,
quote authorization, recovery, physical qualification, and release readiness
remain required.

Stage 24.34 - TPM attestation-key Name binding

Implemented TPM Name recomputation, returned-Name validation, and exact
comparison against a provisioned expected object Name. The executable QEMU
self-test, host tests, Clippy, and signed 1/2/4-CPU acceptance pass. Live
retrieval, identity provisioning, quote authorization, recovery, physical
qualification, and release readiness remain required.

Stage 24.33 - Bounded TPM ReadPublic attestation-key boundary

Implemented bounded TPM2_ReadPublic framing and validation of SHA-256-named
fixed/restricted signing objects with RSA or P-256 public keys. The
executable QEMU self-test, host tests, Clippy, and signed 1/2/4-CPU
acceptance pass. Live TPM execution, object-name binding, persistence,
recovery, physical qualification, and release readiness remain required.

Stage 24.32 - Bounded ECC/P-256 quote verification

Implemented bounded ECDSA/P-256 SHA-256 verification over exact TPMS_ATTEST
bytes. The executable QEMU self-test, host tests, Clippy, and signed
1/2/4-CPU acceptance pass. Live TPM key retrieval/validation, quote-key
binding, recovery, physical qualification, and release readiness remain
required.

Stage 24.31 - Bounded TPM RSA quote verification

Implemented SHA-256 RSASSA and RSAPSS verification over exact TPMS_ATTEST
bytes against a bounded TPM RSA public-key shape. The executable QEMU
self-test, host tests, Clippy, and signed 1/2/4-CPU acceptance pass. ECC
verification, live TPM key retrieval/validation, recovery, physical
qualification, and release readiness remain required.

Stage 24.30 - Bounded TPM quote signature framing

Implemented bounded TPMT_SIGNATURE framing for SHA-256 RSASSA, RSASSA-PSS,
and ECDSA quote schemes. The executable QEMU self-test, host tests, Clippy,
and signed 1/2/4-CPU acceptance pass. Public-key/signature verification,
physical TPM attestation, recovery, and release qualification remain
required.

Stage 24.29 - Bounded TPM quote attestation parsing

Implemented bounded TPM quote parsing and nonce/PCR-digest binding. The
executable QEMU self-test, host tests, Clippy, and signed 1/2/4-CPU
acceptance pass. Quote signature/key verification, live physical TPM
attestation, recovery, and release qualification remain required.

Stage 24.28 - Credential-root read-back promotion boundary

Implemented combined NV metadata and constant-time exact-value validation
before credential-root promotion. The executable QEMU self-test, host tests,
Clippy, and signed 1/2/4-CPU acceptance pass. Live TPM trust, quote
verification, crash recovery, physical qualification, and release custody
remain required.

Stage 24.27 - Policy-session NV read-back

Implemented bounded policy-session `NV_Read` framing and exact 32-byte
read-back response validation. The executable QEMU self-test, host tests,
Clippy, and signed 1/2/4-CPU acceptance pass. Live TPM provisioning, quote
verification, recovery, and physical qualification remain required.

Stage 24.26 - Bounded TPM NV definition framing

Implemented bounded owner-authorized `NV_DefineSpace` framing for the fixed
credential-root metadata. The executable QEMU self-test, host tests, Clippy,
and signed 1/2/4-CPU acceptance pass. Live provisioning, quote verification,
recovery, and physical TPM qualification remain required.

Stage 24.25 - Policy-session NV write framing

Implemented bounded sessions-tagged `NV_Write` framing and response
authorization parsing for the fixed credential-root payload. The executable
QEMU self-test, host tests, Clippy, and signed 1/2/4-CPU acceptance pass.
Live NV provisioning, quote verification, recovery, and physical TPM
qualification remain required.

Stage 24.24 - Bounded TPM PolicyPCR transcript

Implemented bounded TPM2 `PolicyPCR` framing and exact response validation.
The executable QEMU self-test, host tests, Clippy, and signed 1/2/4-CPU
acceptance pass. Live TPM authorization, NV provisioning, quote verification,
recovery, and physical qualification remain required.

Stage 24.23 - Bounded TPM policy-session framing

Implemented bounded TPM2 policy-session start framing and response parsing.
The executable QEMU self-test, host tests, Clippy, and signed 1/2/4-CPU
acceptance pass. This does not authorize NV operations or provision a sealed
credential root; policy-command transcripts, recovery, and physical TPM
qualification remain required.

Stage 24.22 - Credential-root NV policy validation

Implemented fail-closed validation of the reserved NV credential-root
metadata against the expected index and PCR policy. Host tests, Clippy, and
signed 1/2/4-CPU acceptance pass. This does not create, authorize, read, or
write an NV object; authorization sessions, sealed provisioning, quote
verification, recovery, and physical qualification remain required.

Stage 24.21 - Bounded TPM NV public metadata

Implemented bounded, metadata-only `NV_ReadPublic` framing and validation.
Host tests, Clippy, and signed 1/2/4-CPU acceptance pass. No NV index is
created or written, no authorization session is implemented, and no secret or
credential root is exposed. Policy-bound sealing, quote verification,
recovery, physical TPM qualification, and release signing remain required.

Stage 24.20 - Signed PCR policy boundary

Implemented a fixed-size signed PCR policy envelope with caller-provisioned
Ed25519 trust, exact PCR/digest binding, and fail-closed signature/tamper/
mismatch checks. Host, Clippy, and signed 1/2/4-CPU acceptance pass. Runtime
policy provisioning, TPM quote verification, sealed credential roots,
recovery, and physical qualification remain open.

Stage 24.19 - Bounded PCR evidence read

Implemented bounded SHA-256 PCR_Read framing and validation plus a PCR7 boot
inventory probe. Host, Clippy, and signed 1/2/4-CPU acceptance pass. PCR
state is not trusted by presence alone; expected-policy provisioning, signed
quotes, attestation-key validation, sealed credential-root provisioning,
recovery, and physical qualification remain open.

Stage 24.18 - Bounded TPM2 CRB probe

Implemented a serialized, bounded CRB `GetRandom` probe on the validated TPM2
descriptor. Interface/buffer checks, fatal status, fixed pre-interrupt poll
timeout, response validation, and output wiping pass the host/Clippy and
signed 1/2/4-CPU gates. Random output is not a stable credential root;
attestation, sealed provisioning, recovery, and physical qualification remain
open.

Stage 24.17 - TPM2 platform discovery boundary

Implemented validated ACPI TPM2 discovery and explicit boot inventory.
Missing or malformed firmware data fails closed as a trust source and cannot
be mistaken for hardware-backed identity. Host, Clippy, and signed 1/2/4-CPU
acceptance pass. CRB transport, attestation/PCR verification, sealed
credential-root provisioning, recovery, and physical qualification remain
open.

Stage 24.16 - Durable/configurable lockout policy

Implemented the bounded persisted failed-authentication threshold (1..=32),
backward-readable WID1 snapshots, syscall 232/audit action 37, runtime policy
application, and the WovenSecurity administrator editor. Host, Clippy,
focused UI QEMU, and signed 1/2/4-CPU acceptance pass. Hardware credentials,
malformed-request telemetry, release-key provisioning, and physical
qualification remain open.

Stage 24.15 - Administration mutation UI

Implemented the complete bounded WovenSecurity mutation flow: add user, add
group, add/remove supplementary membership, and delete inactive user. Focused
UI and signed 1/2/4-CPU acceptance pass. The disposable hardware-demo image
uses volatile state only when no WovenFS volume exists; production remains
durable and fail-closed.

Stage 24.14 - Bounded administration query UI

Implemented the bounded identity user/group query ABI, audit action 36, and
the WovenSecurity elevated administrator directory flow. Host, Clippy, focused
UI QEMU, and signed 1/2/4-CPU acceptance pass. This stage remains a query
surface; mutation UI, configurable durable lockout, hardware credentials,
malformed-request telemetry, release-key provisioning, and physical
qualification remain prerequisites for a production release.

Stage 24.13 - Group membership administration

Implemented bounded elevated-admin supplementary group membership mutation (syscalls 229-230), with duplicate/full/unknown identity checks, primary-group protection, WID2 publication, rollback, and security-ledger actions. Host, Clippy, and signed 1/2/4-CPU acceptance pass.

Next prerequisites are durable/configurable lockout policy, hardware credentials, malformed-request telemetry, release-key provisioning, and physical qualification.

Current stage

Complete:

asynchronous TCP connect
asynchronous TCP send
asynchronous TCP receive
EOF handling
connection-state transitions
socket generation/pinning
close while operations are in flight
cancellation
process teardown
SMP-safe network worker
1/2/4 CPU testing
real QEMU host↔guest TCP tests

Exit condition: Stage 10.7 acceptance passes completely.

Stage 10.8 — Unified Completion Port / Wait-Many

This should be the next major architectural feature.

Implement a per-process completion mechanism similar conceptually to:

epoll
kqueue
IOCP
io_uring completion queues

A process must be able to wait for many asynchronous operations simultaneously.

Example:

File read ─────┐
TCP receive ───┤
Timer ─────────┼──> Completion Port ──> Application
IPC message ───┤
Block I/O ─────┘

Implement:

CompletionPort object
completion queue
associate async operation with port
wait-one
wait-many
batch completion dequeue
cancellation events
timeout support
generation-safe handles
SMP-safe producer/consumer behavior
bounded queues
overflow behavior
owner teardown

This becomes the basis for GUI event loops, servers and AI services.

Stage 10.9 — Timers + Asynchronous Events

Implement proper application-visible:

one-shot timers
periodic timers
monotonic clock
deadlines
timeout cancellation
sleep-until
timer completion events
event objects
wait-many integration

No application should need busy waiting.

Stage 10.10 — Mature Async Runtime

Unify:

BLOCK
FILE
TCP
UDP
IPC
TIMERS
DEVICES
EVENTS

under the Stage 10.8 completion architecture.

At this point Stage 10 is closed.

Phase II — Process and userspace architecture
Stage 11 — Production Userspace
11.1 — Process Model

Develop a real process abstraction:

Process
 ├── Address Space
 ├── Threads
 ├── Capability Table
 ├── File Handles
 ├── IPC Handles
 ├── Completion Ports
 ├── Security Domain
 └── Resource Limits

Add:

PID management
parent/child relationships
exit status
waitpid-like semantics
process groups
clean teardown
11.2 — Multithreaded Processes

Implement:

thread creation
thread termination
join
TLS
per-thread stack
per-thread registers
thread-local errno/state
11.3 — Signals / Structured Notifications

Rather than blindly cloning Unix signals, design safer WovenHat notifications.

Support:

terminate
suspend
resume
child exit
exceptions
user-defined notifications

Prefer structured event delivery.

11.4 — Userspace Runtime

Create:

libwoven

The fundamental WovenHat userspace library.

Eventually expose:

woven::process
woven::thread
woven::fs
woven::net
woven::ipc
woven::async_io
woven::security
woven::time
woven::graphics
11.5 — Program Loader

Strengthen ELF loading:

PIE
ASLR
dynamic relocation
shared libraries
TLS
stack protection
W^X
RELRO-style protection
Phase III — Filesystem and storage
Stage 12 — Production Storage Stack
12.1 — VFS 2.0

Build a clean filesystem abstraction:

Application
     ↓
VFS
 ┌───┼────┬───────┐
WFS FAT tmpfs procfs
12.2 — WovenFS

Create WovenHat's native filesystem.

Target features:

64-bit filesystem
directories
metadata
timestamps
permissions
extended attributes
checksums
journaling or copy-on-write
crash consistency
12.3 — Filesystem Encryption

Add:

encrypted volumes
per-user keys
secure key handling
authenticated encryption

The kernel now provides the RFC 8439 authenticated-encryption envelope, a
bounded generation-safe revocable key vault, authenticated persistent wrapped-key
records, an explicit fail-closed measured-provisioning boundary, crash-safe
successor-generation rotation, and encrypted mount-record I/O bound to volume,
generation, mount, owner, and opaque vault authority.

The production integration gate covers tampered persistent records, untrusted
measurement evidence, cross-volume/generation rotation rejection, mounted-record
authentication failure without plaintext mutation, key revocation/stale mount
authority, and 1/2/4-core release validation. A platform TPM/firmware backend is
still required to supply real measured-root evidence on physical hardware; the
kernel does not substitute best-effort entropy for that trust source.
12.4 — Snapshots

Implement filesystem snapshots.

This enables one of WovenHat's future signature features:

Undo System Change

If an update or application damages the system, WovenHat can roll back.

12.5 — Storage Management

Implement:

partitions
GPT
mount manager
removable media
filesystem detection
disk information
Phase IV — Driver architecture
Stage 13 — WovenDriver Framework

Do not attempt to write a separate driver for every device on Earth.

Instead build a reusable driver architecture.

13.1 — Driver Framework

Create:

Driver Manager
      ↓
Device Bus
 ├── PCIe
 ├── USB
 ├── ACPI
 └── Virtual
      ↓
Drivers

Support:

device discovery
matching
binding
initialization
hotplug
suspend/resume
driver isolation
13.2 — PCIe

Production PCI/PCIe support.

13.3 — NVMe

Native NVMe driver.

13.4 — AHCI/SATA

For SATA SSDs/HDDs.

13.5 — USB Core

Implement:

xHCI
USB enumeration
endpoints
transfers
hotplug
13.6 — USB HID

Support:

keyboards
mice
touchpads
13.7 — Input Framework

Unified:

Keyboard
Mouse
Touchpad
Touchscreen
Pen
Game controller
       ↓
WovenInput
13.8 — Audio

Create WovenAudio.

Initially support:

Intel HDA
playback
recording
volume
streams
13.9 — Wi-Fi Framework

Start with selected hardware families rather than every Wi-Fi adapter.

Implement:

scanning
association
WPA2/WPA3 integration
network selection
reconnect
13.10 — Bluetooth

Later add:

Bluetooth controller
HID
audio
pairing
Phase V — Networking
Stage 14 — WovenNet

Take the existing networking work to production level.

14.1

Stable:

UDP
TCP
DNS
DHCP
ICMP
14.2 — ACCEPTED

Add:

IPv6
DHCPv6
IPv6 neighbor discovery

Acceptance status: complete in the Stage 14.2 bounded kernel/networking scope.
The accepted gate covers IPv6 address/prefix foundations, ICMPv6/NDP parsing
and checksum validation, router-advertisement state, neighbor/DAD state,
SLAAC lifecycle, DHCPv6 protocol/client/lease state, IPv6 interface ingress,
runtime link-local interface installation, and hardened NDP ingress rules
including Hop Limit 255 and source-address validation. Physical-NIC IPv6
qualification, broader live IPv6 socket policy, and routing ownership remain
separate follow-up/Stage 14.3+ concerns.

14.3 — ACCEPTED

Socket API.

Acceptance status: complete in the bounded WovenNet socket-API scope. The
accepted boundary preserves the legacy packed IPv4 ABI while adding a
versioned IPv4/IPv6 endpoint representation, routes that representation
through real connect/peer operations, and retains owner-scoped descriptors,
generation-bound async pinning, stale-token rejection, bounded slot reuse,
and process socket cleanup. The mandatory Stage 14.3 authority/lifecycle gate
is inherited by downstream networking tests. A complete POSIX/BSD API,
listening/accept ergonomics beyond the current kernel primitives, and
physical-network IPv6 interoperability remain later platform hardening.

14.4 — ACCEPTED

Routing tables.

Acceptance status: complete in the bounded WovenNet routing-policy scope. The
accepted route table is dual-stack for IPv4 and IPv6, normalizes prefixes,
performs longest-prefix selection with metric/generation tie-breaking, and
uses owner-scoped, generation-tagged route authority. Cross-owner removal is
rejected and owner teardown can revoke all owned routes deterministically.

The policy boundary is connected to the live smoltcp route set through
controlled installation and exact removal. Acceptance proves idempotent live
installation and stale second-removal rejection for both IPv4 and IPv6 while
preserving the existing network configuration. The Stage 14.4 QEMU gate runs
on the inherited networking acceptance matrix.

This acceptance does not claim a userspace route-management service,
dynamic routing protocols, policy routing across multiple interfaces, or
physical-network interoperability across all NIC families. Those remain
later Network Manager/platform hardening concerns.

14.5

WovenGuard firewall.

14.6

TLS userspace library.

14.7

HTTP/HTTPS client.

14.8

Network Manager service.

Eventually the desktop gets:

Wi-Fi
Ethernet
VPN
DNS
Firewall
Hotspot

from one userspace service.

Phase VI — WovenGuard Security Architecture
Stage 15 — WovenGuard 2.0

You already have an unusually strong security foundation. This phase turns it into a complete OS security architecture.

15.1 — Capability Enforcement Everywhere

Everything sensitive becomes capability-controlled:

Camera
Microphone
Network
Filesystem
GPU
USB
Clipboard
Location
Processes
IPC
AI
15.2 — Application Sandboxes

Each application gets its own security domain.

Example:

Browser
 ├── Network ✓
 ├── Downloads ✓
 ├── Camera ASK
 ├── Microphone ASK
 ├── System files ✗
 └── Other apps' memory ✗
15.3 — Permission Broker

Graphical permission prompts.

15.4 — Signed Applications

Verify software signatures before execution.

15.5 — Secure Boot

Eventually support:

UEFI Secure Boot
      ↓
WovenHat Bootloader
      ↓
Verified Kernel
      ↓
Verified System
15.6 — Measured Boot

TPM-backed measurements.

15.7 — Secrets Vault

Build:

WovenVault

for:

passwords
tokens
encryption keys
Wi-Fi credentials
application secrets
15.8 — Security Ledger

Expand your existing audit ledger into a user-visible security history.

For example:

10:42 Browser requested camera
10:42 Permission granted
10:47 Camera capability revoked
11:03 Unknown USB device blocked
15.9 — Exploit Mitigations

Implement or verify:

ASLR
NX
SMEP
SMAP
stack guards
guard pages
W^X
heap hardening
kernel address randomization
Phase VII — Graphics
Stage 16 — WovenGraphics

This is where WovenHat begins becoming a graphical OS.

16.1 — Graphics Abstraction

Stop treating UEFI framebuffer as the final graphics architecture.

Create:

WovenGraphics
     ↓
Display Driver
     ↓
GPU / framebuffer
16.2 — 2D Renderer

Support:

rectangles
paths
gradients
images
clipping
transforms
alpha blending
16.3 — Font Engine

Implement:

TrueType/OpenType
Unicode
font fallback
anti-aliasing
text shaping

Eventually integrate complex scripts correctly.

16.4 — GPU Acceleration

Initially use software rendering.

Then introduce:

virtio-gpu
hardware-specific GPU work
accelerated surfaces

Do not make NVIDIA/AMD/Intel native GPU support a prerequisite for the first GUI release.

Phase VIII — Window System
Stage 17 — WovenCompositor

This is one of the most important stages.

Architecture:

Applications
     ↓
WovenSurface Protocol
     ↓
WovenCompositor
     ↓
WovenGraphics
     ↓
GPU

Implement:

windows
surfaces
compositing
damage tracking
z-order
transparency
shadows
animations
input routing
focus
multiple monitors
DPI scaling

Security rule:

Applications must never directly control another application's surface.

Phase IX — WovenHat Desktop
Stage 18 — WovenShell

Now build the desktop environment.

8
18.1 — Desktop

Build:

wallpaper
desktop surface
taskbar/dock
system tray
clock
notifications
18.2 — WovenStart

Application launcher/search.

18.3 — Window Experience

Develop WovenHat's own visual identity rather than simply copying Windows.

Consider:

dynamic depth
subtle translucency
adaptive surfaces
smooth physics-based animation
workspace transitions
intelligent tiling
18.4 — Notification Center
18.5 — Quick Settings

Wi-Fi, Bluetooth, sound, brightness, power, VPN and security.

18.6 — Lock Screen
18.7 — Login Manager
18.8 — Multiple Workspaces
18.9 — Multi-monitor Desktop
Phase X — Core Applications
Stage 19 — WovenHat System Applications

Develop native applications:

WovenFiles
WovenTerminal
WovenSettings
WovenMonitor
WovenSecurity
WovenEditor
WovenCalculator
WovenImages
WovenUpdate
WovenStore

The Settings application should expose:

appearance
accounts
networking
devices
applications
privacy
security
updates
storage
accessibility
Phase XI — Application Platform
Stage 20 — WovenSDK

Applications need a stable development platform.

Build:

SDK
application runtime
GUI toolkit
widgets
layout system
accessibility API
notification API
clipboard API
drag-and-drop
file chooser
permission API

Create something conceptually like:

Application::new("My App")
    .window(...)
    .run();
Stage 20.5 — WovenUI

Your official native GUI toolkit.

Components:

Button
Text
TextField
List
Table
Tree
Menu
Dialog
Tabs
Navigation
Image
Video
Canvas
WebView
Phase XII — Software Distribution
Stage 21 — WovenPackages

Build a package format.

For example:

example.woven

Include:

manifest
application
resources
capabilities
signature
version
dependencies

Then build:

WovenStore

with:

installation
updates
removal
signature validation
rollback
sandbox declarations
Phase XIII — System Services
Stage 22 — Service Architecture

Move functionality out of the kernel wherever practical.

Create services such as:

WovenNetwork
WovenAudio
WovenDisplay
WovenInput
WovenPackage
WovenUpdate
WovenSecurity
WovenIdentity
WovenAI

IPC becomes the backbone.

This keeps the kernel smaller and makes components independently replaceable.

22.5 — WovenPackage transaction authority

Move package mutation authority to the Ring-3 WovenPackage broker. The broker
must use bounded IPC control frames, resolve package sources through its own
VFS namespace, require explicit read/write/storage capabilities, authenticate
WPKG envelopes before mutation, and commit install, removal, rollback, and
recovery through durable WovenFS catalog/journal records. Acceptance requires
the transaction path on 1/2/4 CPUs; physical USB qualification and complete
update-daemon ownership remain separate release prerequisites.

Phase XIV — AI-Native WovenHat
Stage 23 — WovenAI

AI should not merely be a chatbot installed on the OS.

Make AI an OS service.

Architecture:

Applications ──┐
Desktop ───────┤
Search ────────┤
Accessibility ─┼── WovenAI Broker
Terminal ──────┤        ↓
Files ─────────┤   AI Providers
Security ──────┘
23.1 — AI Broker

Applications request AI capabilities through controlled IPC.

The first boundary must be a bounded Ring-3 WovenAI broker with a versioned
control protocol and a kernel syscall that requires both `Ipc` and `Ai`
capability. The broker may describe its ABI and report that no provider is
installed, but it must not claim model execution, file access, network access,
secret access, or device access until later provider and security stages are
accepted.

23.2 — Local Models

Support local inference where hardware permits. The first accepted provider is
the bounded, deterministic Ring-3 local provider: fixed integer weights,
bounded UTF-8 input, no allocation, an explicit model ABI, and no implicit
file, network, secret, or device access. Larger model formats and hardware
acceleration remain separate provider work; this stage must not imply general
neural inference capability.

23.3 — Cloud Models

Provider plugins, but credentials stay in WovenVault. The accepted foundation
is a separate Ring-3 provider ABI with broker delegation and owner-bound vault
credentials; real HTTPS/TLS endpoint support, provisioning, retry, quota, and
availability policy must be accepted as provider hardening before release.

23.4 — AI Capability Security

An AI model gets no automatic access to:

files
camera
microphone
passwords
applications
network

It receives explicit WovenGuard capabilities.

Stage 23.4 acceptance requires the local and provider profiles to narrow
authority through WovenGuard file/device policy; no model inherits secrets,
files, camera, microphone, USB, applications, or network access without an
explicit package capability declaration.

23.5 — Semantic Search

Search:

Applications
Files
Settings
Messages
Documents
Commands

Stage 23.5 begins with a bounded fixed-capacity index and deterministic term
scoring; ingestion, privacy filtering, durable updates, and richer semantic
ranking remain explicit acceptance work rather than implicit capabilities.
23.6 — Woven Assistant

System assistant capable of executing permitted OS actions.

The accepted foundation is a bounded action planner with explicit permission
and confirmation checks. The user-facing permission broker, audit-visible
confirmation flow, cancellation, and platform executors remain required for
the complete assistant stage.

23.7 — AI Security Monitor

AI can explain suspicious activity, but WovenGuard remains the actual enforcement mechanism.

That distinction is important: AI advises; deterministic kernel/security policy enforces.

Stage 23.7 acceptance therefore permits advisory summaries only; anomaly
detection, notification UX, and response automation cannot bypass WovenGuard.

Phase XV — Accounts and Identity
Stage 24 — WovenIdentity

Stage 24.60 - Release-key provisioning boundary
Production WAPP catalogs and the package service require explicit release
signing and trusted public-key inputs; development-key fallback is limited to
non-production packaging. The packager derives and checks the public key, and
release packaging propagates the fail-closed production mode.

Stage 24.46 boot-time TPM context recovery accepted (2026-10-21): after
WovenFS mount, system-apps boot treats the newest validated `TPS1` record as a
candidate only, requires live TPM2 `ContextLoad`, and flushes the restored
lease while hardware credential authority remains disabled. Missing, invalid,
or rejected custody fails closed. Host, Clippy, memory QEMU, and signed
system-apps 1/2/4-CPU gates pass. Physical cross-reboot restore, secure
storage authorization, nonce refresh, provisioned-key lifecycle, credential
decisions, and physical qualification remain open.

Stage 24.1 foundation accepted (2026-10-20): `libwoven::identity` provides a
bounded local users/groups/session policy core with PBKDF2 credentials,
profiles, lock/unlock, and password-confirmed elevation. The gate passes on
1/2/4 CPUs. Kernel credential switching, durable identity persistence,
authorization-broker integration, lockout policy, administration UI, and
hardware-backed credentials remain open; do not treat this foundation as a
complete production identity implementation.

Stage 24.2 authenticated snapshot accepted (2026-10-20): users, groups,
administrator roles, profiles, salts, and password digests have a bounded
authenticated `WID1` representation; sessions are intentionally excluded.
Kernel-authorized durable filesystem writes/recovery, key management,
credential rotation, lockout, and task/session integration remain open.

Stage 24.3 durable storage accepted (2026-10-20): the kernel writes and
recovers authenticated dual-copy `WID2` identity records through the
StorageIo-authorized native WovenFS path. User-facing persistence, key
management, rotation, lockout, and task binding remain open.

Stage 24.4 owner-bound kernel identity seam accepted (2026-10-20): fixed
process-owned session bindings and a crate-only credential application seam
are present, with fail-closed cleanup when no live owner exists. Userspace
syscalls, durable runtime loading, exit reclamation, lockout, and shell
integration remain open.

Stage 24.5 syscall ABI accepted (2026-10-20): syscalls 219–223 provide bounded
session open/lock/unlock/elevation/close framing and safe userspace wrappers.
The gate proves parser/ABI behavior only; runtime loading, live Ring-3 login,
teardown, auditing, lockout, and shell migration remain open.

Stage 24.12 durable identity administration accepted (2026-10-20): admin user
and group mutations publish through WID2 and synchronize the trusted account
file with rollback on failure; the SMP stress-slot race is fixed. Host,
feature-gate, and 1/2/4-CPU signed system-app gates pass. Group membership,
UI integration, configurable lockout, hardware credentials, malformed
telemetry, release-key provisioning, and physical qualification remain open.

Stage 24.11 bounded identity administration ABI accepted (2026-10-20):
elevated administrator sessions can add users, delete inactive users, and add
groups through bounded syscalls 226–228 with stable audit actions 31–33. Host,
feature-gate, and 1/2/4-CPU signed system-app gates pass. Durable account/WID2
publication, group membership mutation/deletion, and administration UI remain
open.

Stage 24.10 kernel identity lockout/rate limiting accepted (2026-10-20):
plaintext and verified-digest authentication share a bounded five-failure,
five-second cooldown policy, with success reset and fail-closed table
exhaustion. Host, feature-gate, and 1/2/4-CPU signed system-app gates pass.
Durable/configurable lockout state, account administration, malformed-request
telemetry, hardware credentials, release-key provisioning, and physical
qualification remain open.

Stage 24.9 live PIN rotation accepted (2026-10-20): the account-change path
updates the owner-bound kernel authority, trusted account file, and shell cache
with rollback on file failure; host, feature-gate, and 1/2/4-CPU gates pass.
Account administration/deletion, lockout/rate limiting, malformed-request
telemetry, hardware credentials, release-key provisioning, and physical
qualification remain open.

Stage 24.8 WID2 runtime recovery accepted (2026-10-20): system-apps startup
prefers the newest authenticated native WID2 snapshot and falls back safely to
the bounded account file when no durable copy exists. Stage 24.3 native
storage, host, system-apps, and 1/2/4-CPU gates pass. Hardware credential roots,
administration/rotation, lockout/rate limiting, malformed-request telemetry,
release-key provisioning, and physical qualification remain open.

Stage 24.7 identity audit actions accepted (2026-10-20): valid identity open,
lock, unlock, elevation, and close outcomes are recorded in the kernel security
ledger and decoded through stable public action IDs 26–30. Host, freestanding,
and 1/2/4-CPU signed system-app gates pass. Malformed-request telemetry,
durable WID2 runtime recovery, account administration, PIN rotation,
lockout/rate limiting, and hardware-backed credentials remain open.

Stage 24.6 live runtime/login accepted (2026-10-20): the system-apps boot
imports bounded account-file credentials into the kernel authority, WovenShell
performs live account sign-in and lock/unlock through owner-bound sessions,
credentials are applied to the shell process, and owner teardown reclaims
bindings. The 1/2/4-CPU signed system-app/hardware-demo gate passes. WID2
runtime recovery, account administration, PIN rotation, lockout/rate limiting,
identity audit actions, and hardware-backed credentials remain open.

Implement:

local users
passwords
groups
sessions
lock/unlock
privilege elevation
user profiles
credential storage

Later:

biometrics
security keys
FIDO2/WebAuthn
enterprise identity
Phase XVI — Power and Hardware Management
Stage 25 — Power Management

Stage 25.1 - ACPI power capability boundary

Accepted bounded slice: discover firmware `_S3` sleep types alongside `_S5`
and expose a privileged, read-only capability query (syscall 235 and the
`libwoven` wrapper). This does not perform suspend/resume; driver save/restore,
wake-source handling, and physical qualification remain prerequisites for the
later sleep/wake implementation.

Stage 25.2 - CPU idle capability boundary

Accepted bounded slice: report architectural `HLT`, optional `MONITOR/MWAIT`,
and invariant-TSC support through a privileged read-only query (syscall 236
and the `libwoven` wrapper). The scheduler remains on the tested `STI+HLT`
path; MWAIT, frequency MSRs, deep C-state policy, and residency accounting
remain later work.

Implement:

Stage 25.3 - CPU frequency inventory boundary

Accepted bounded slice: read CPUID leaf `0x16` when present and expose base,
maximum, and bus MHz inventory through a privileged read-only query (syscall
237 and the `libwoven` wrapper). No P-state or frequency-control MSR is
written; governor policy, thermal interaction, and physical validation remain
later work.

Stage 25.4 - bounded suspend preparation

Accepted bounded slice: add a reversible preparation transaction that checks
ACPI S3 capability, quiesces bound drivers in deterministic order, and rolls
back in reverse order when a driver refuses. This does not write `SLP_EN`,
enter S3, install a wake vector, coordinate CPU resume, or expose a suspend
syscall; those remain prerequisites for the actual sleep/wake implementation.

Stage 25.5 - ACPI FACS wake-capability boundary

Accepted bounded slice: validate the FADT-referenced FACS and report bounded
S3/FACS/waking-vector readiness through a privileged read-only query (syscall
238 and the `libwoven` wrapper). No firmware address is exposed and no FACS,
CPU, or PM1 state is written; wake-trampoline and actual sleep/wake work
remain prerequisites.

Stage 25.6 - low-memory wake-page reservation

Accepted bounded slice: reserve a distinct usable page below 640 KiB for the
future S3 wake trampoline, separate from the AP startup page and unavailable
to the general allocator. This does not write trampoline code, install a FACS
waking vector, enter S3, or handle firmware wake.

Stage 25.7 - wake-trampoline staging

Accepted bounded slice: stage the existing AP transition image on the reserved
low page with a private identity map, dedicated stack, and transactional
page-table ownership. This does not install the FACS vector, save CPU state,
enter S3, or handle firmware wake.

Stage 25.8 - CPU resume-context capture

Accepted bounded slice: capture and validate the current CPU control state and
descriptor tables in kernel-owned suspend preparation storage. This does not
restore registers, coordinate secondary CPUs, install a FACS vector, enter S3,
or claim firmware or physical suspend/resume qualification.

Stage 25.9 - transactional FACS wake-vector installation

Accepted bounded slice: install the reserved low wake-page address in the
validated 32-bit and 64-bit FACS waking-vector fields, retain their previous
values in the private preparation token, and restore them on abort. This does
not coordinate secondary CPUs, restore CPU registers, write PM1 S3 controls,
or claim firmware or physical suspend/resume qualification.

Stage 25.10 - secondary-CPU suspend barrier

Accepted bounded slice: use the existing scheduler/hotplug ownership path to
evacuate and park all online secondary CPUs, retain the exact online mask, and
restore every parked CPU on rollback. This does not restore CPU registers,
write PM1 S3 controls, perform firmware wake, or claim physical qualification.

Stage 25.11 - PM1 S3 request boundary

Accepted bounded slice: encode the validated `_S3` types and write PM1a and
optional PM1b control words with `SLP_EN` only after the complete preparation
transaction commits. If the machine remains running, roll back every owned
resource and return an explicit error. The gate does not issue a destructive
sleep request; CPU restore, firmware wake, and physical qualification remain
later work.

Stage 25.12 - CPU resume-context restoration primitive

Accepted bounded slice: restore the validated kernel-owned CPU context and
transfer to a kernel continuation without returning through a stale firmware
stack. This does not yet wire a persistent firmware-wake record, restore
drivers after S3, or claim physical suspend/resume qualification.

Stage 25.13 - persistent firmware-wake record integration

Accepted bounded slice: retain the committed suspend preparation in a
kernel-owned record, restore its CPU context from the low-memory wake entry,
and transfer to a continuation that performs owned rollback and driver/CPU
restoration. This does not claim that firmware has actually completed S3 wake
or that physical suspend/resume is qualified.

Stage 25.14 - suspend admission boundary

Accepted bounded slice: add the privileged suspend action to the stable power
ABI, but fail closed with a deterministic error until the wake path can return
to the interrupted userspace syscall frame. The rejected request performs no
driver quiescence, CPU parking, FACS write, or PM1 write. A later stage must
commit a task-owned resume target and verify its return path before enabling a
real user-facing S3 transition.

ACPI poweroff
reboot
sleep
wake
CPU idle
CPU frequency management
battery
thermal monitoring
laptop lid
suspend/resume
Phase XVII — Accessibility and Internationalization
Stage 26 — Accessibility

Accessibility must be architectural rather than added at the end.

Implement:

screen reader APIs
keyboard navigation
high contrast
scaling
magnifier
reduced motion
captions
accessibility tree
Stage 26.5 — Internationalization

Implement:

Unicode
locale
date/time formats
number formats
translations
RTL
keyboard layouts
IMEs

This also creates a path for excellent African-language support, including Chichewa.

Phase XVIII — Updates and Recovery
Stage 27 — WovenUpdate

Updates should be:

signed
atomic
rollback-safe
resumable

Potential architecture:

Current System A
       ↓
install update
       ↓
System B
       ↓
reboot
       ↓
verify
   ↙       ↘
PASS       FAIL
 ↓          ↓
keep B    rollback A
Stage 27.5 — Recovery Environment

Build:

WovenRecovery

capable of:

repairing boot
filesystem checking
restoring snapshots
resetting system
reinstalling
retrieving logs
Phase XIX — Installation
Stage 28 — WovenInstaller

Turn the development image into something people can install.

Implement:

bootable ISO/USB image
graphical installer
disk detection
partitioning
filesystem formatting
bootloader installation
user creation
timezone
keyboard
networking
installation progress
Phase XX — Hardware Expansion
Stage 29 — Real Hardware Qualification

Create a hardware compatibility laboratory.

Test categories:

Intel PCs
AMD PCs
Laptops
Desktops
NVMe
SATA
USB
Ethernet
Wi-Fi
Audio
Intel graphics
AMD graphics
NVIDIA graphics
Touchpads
Webcams
Bluetooth
Multi-monitor

Do not require every device to work before 1.0.

Instead establish:

Tier 1 — officially supported

Tier 2 — expected to work

Tier 3 — experimental

Phase XXI — Performance
Stage 30 — Performance Engineering

Build proper tooling:

kernel profiler
scheduler latency measurements
syscall benchmarks
IPC benchmarks
filesystem benchmarks
network throughput
compositor frame timing
memory-pressure testing
boot profiling

Targets should eventually include:

60/120 Hz desktop
low IPC latency
fast boot
low idle CPU
controlled memory usage
responsive application startup
Phase XXII — Reliability
Stage 31 — WovenHat Torture Suite

This is critical before calling the OS finished.

Automate:

1 CPU
2 CPUs
4 CPUs
8 CPUs
16 CPUs

Stress:

scheduler
memory
IPC
capabilities
filesystems
networking
async I/O
drivers
GUI
process lifecycle

Include randomized failure injection.

Test:

low memory
disk full
network loss
corrupted packets
driver crash
process crash
service restart
sudden shutdown

Run thousands of automated boots.

Phase XXIII — Developer Experience
Stage 32 — WovenDev

Create:

debugger
kernel debugger
crash dumps
structured logging
tracing
performance profiler
SDK documentation
API documentation
examples
application templates

Eventually developers should be able to write WovenHat software without understanding the kernel.

Phase XXIV — Release Engineering
Stage 33 — Alpha

The OS should now:

boot real hardware
install itself
show the desktop
run applications
connect to networks
play audio
manage files
update itself

Expect bugs.

Stage 34 — Beta

Focus on:

compatibility
stability
hardware
accessibility
security audit
performance
application polish

Feature freeze begins.

Stage 35 — Release Candidate

No major architecture changes.

Only:

critical bug fixes
security fixes
compatibility fixes
performance regressions
documentation

Run full acceptance repeatedly.

Stage 36 — WovenHat OS 1.0

This is the point I would call the first version of WovenHat finished enough to be a real general-purpose operating system.

The 1.0 stack would look approximately like this:

┌─────────────────────────────────────────────┐
│               WovenHat Desktop              │
│ WovenFiles │ Settings │ Store │ Assistant   │
├─────────────────────────────────────────────┤
│        WovenUI + WovenSDK + WovenAI         │
├─────────────────────────────────────────────┤
│ WovenCompositor │ Audio │ Network │ Update  │
├─────────────────────────────────────────────┤
│           Userspace System Services         │
├─────────────────────────────────────────────┤
│       WovenGuard Security Architecture      │
├─────────────────────────────────────────────┤
│ IPC │ Async I/O │ VFS │ Processes │ Sockets│
├─────────────────────────────────────────────┤
│ Scheduler │ VM │ SMP │ Timers │ Interrupts │
├─────────────────────────────────────────────┤
│ PCIe │ NVMe │ USB │ HID │ GPU │ Wi-Fi      │
├─────────────────────────────────────────────┤
│             WovenHat Rust Kernel            │
├─────────────────────────────────────────────┤
│                  Hardware                   │
└─────────────────────────────────────────────┘
Instructions to put above this roadmap when feeding it to Codex

Give Codex this as the permanent operating rule:

WOVENHAT OS MASTER DEVELOPMENT DIRECTIVE

Continue development of WovenHat OS according to the supplied Stage 10.7–36
roadmap.

Do NOT skip stages.

For every stage:

1. Inspect the complete existing repository before modifying it.
2. Preserve all previously accepted architecture and behavior.
3. Implement the stage as production architecture, not a test-only hack.
4. Prefer safe Rust. Keep unsafe Rust small, documented and auditable.
5. Use assembly only where direct CPU architecture requires it.
6. Do not weaken WovenGuard or grant broad capabilities merely to make tests pass.
7. Do not use busy loops where event-driven blocking is appropriate.
8. Avoid global locks where they damage SMP scalability.
9. Protect all kernel objects against stale handles, generation reuse,
   process teardown and concurrent access.
10. Every userspace pointer must be validated and must never be retained
    asynchronously without copying/pinning through a safe kernel-owned mechanism.
11. Test failure paths as well as successful paths.
12. Preserve 1/2/4 CPU acceptance and expand to 8+ CPUs as the SMP architecture matures.
13. Every new stage must run all relevant previous-stage regression tests.
14. Never hide failures by merely increasing timeouts, disabling tests,
    suppressing warnings or weakening assertions.
15. cargo build must pass.
16. cargo clippy -- -D warnings must pass.
17. Host/unit tests must pass.
18. QEMU acceptance tests must pass.
19. Automatically preserve serial/QEMU/failure logs.
20. Do not ask the developer to repeatedly diagnose problems that can be
    determined from source and generated logs.
21. When a failure occurs, inspect the evidence, identify the root cause,
    fix it and rerun automated validation.
22. Update the WovenHat OS Architecture & Codebase Guide for every stage.
23. Document every new file, important structure, syscall, capability,
    subsystem relationship and control/data flow.
24. Update the roadmap/status document after every accepted stage.
25. Commit each accepted stage separately to Git with a descriptive commit.
26. Do not proceed to the next stage until the current stage's acceptance
    criteria are satisfied.

For each completed stage produce:

- implementation summary
- architecture explanation
- files added
- files modified
- syscalls/API changes
- security implications
- WovenGuard implications
- concurrency/SMP analysis
- tests added
- acceptance results
- known limitations
- next-stage prerequisites
- Git commit hash

The objective is not merely to make WovenHat boot.

The objective is to produce a secure, memory-safe, SMP-capable,
event-driven, graphical, AI-integrated, installable and maintainable
general-purpose operating system whose architecture can continue evolving
after WovenHat OS 1.0.
