# WovenHat OS Architecture & Codebase Guide

## WovenDev GDB session lifecycle boundary (2026-10-28)

`kernel/src/gdb_session.rs` owns only the bounded stopped/running/active
state around the pure command dispatcher. It emits protocol stop replies,
ignores commands while running, and exposes continue, step, and kill as
actions for a future trap/task owner. It does not poll UART, dereference
guest memory, mutate registers, or switch tasks; those authorities remain
separate and must be integrated with explicit teardown and cancellation
rules.

## WovenDev GDB context and command boundary (2026-10-28)

`kernel/src/gdb_debugger.rs` keeps GDB command parsing separate from UART and
trap state. A stopped-context value is copied into a fixed 24-register
x86-64 representation, while `m` reads only a caller-owned bounded window;
arbitrary guest addresses are never dereferenced by this layer. Continue,
step, and kill are classified as actions for a future session owner and do not
execute from the pure dispatch boundary.

## WovenDev second-UART boundary (2026-10-27)

`kernel/src/debug_serial.rs` owns only COM2 and is independent of the COM1
kernel log. Its transmit path has a bounded poll budget, receive is
nonblocking, and the Stage 32.9 loopback test restores normal modem-control
mode before emitting its marker. GDB framing, trap handling, and debug-session
authority remain separate layers.

## WovenShell resize surface handoff (2026-10-27)

The shell's `WSIZ`/`WRSZ` protocol permits an owner to release the old shared
surface before publishing its replacement. During this bounded handoff,
repaint blits may fail transiently; the shell suppresses only those expected
errors until the existing resize grace deadline. Client liveness handling and
post-deadline ownership failures remain unchanged and fail closed.

## Stage 20 desktop acceptance revalidation (2026-10-27)

The current WovenShell desktop uses the bounded flat compatibility renderer
for the Stage 20 software interaction gates. The launcher, window movement
and teardown, translucent overlays, tiling, notification center, and
workspace transitions were revalidated against independent kernel pixel and
input oracles on 1/2/4 CPUs. The production 3D path remains separately
qualified by its dedicated gates; this revalidation does not replace native
hardware or full roadmap-stage qualification.

## Scalable userspace I/O contract (2026-10-25)

Kernel read/write syscalls continue to validate each pointer and cap one
transfer to a safe chunk. Applications must use `libwoven::read_all`,
`write_all`, or `write_file_all` for payloads larger than one syscall; these
helpers make progress across the complete caller buffer without introducing a
second artificial application limit. A zero-progress result stops the stream
to avoid hangs. These APIs provide scalability without weakening the kernel's
memory-safety boundary.

## Graphics roadmap Stage 07 image/asset increment (2026-10-25)

`libwoven::media` owns the bounded image decode boundary. It rejects dimensions
above 8192 or 16 megapixels, recognizes the supported container headers, and
provides caller-buffer PNG/JPEG RGBA adapters plus uncompressed 24/32-bit BMP
decoding after validating all offsets and strides. The fixed thumbnail cache,
license/digest wallpaper catalog, and `load_vfs_asset` VFS read helper keep
ownership and capacity explicit. The guest helper reads in 1024-byte chunks to
match the kernel syscall cap. The production kernel embeds the manifest-listed
CC0-1.0 wallpaper and installs it read-only in the boot VFS at
`/share/wallpapers/wovenhat-gradient.bmp`; WovenImages decodes that asset and
renders its bounded thumbnail. WebP pixel decoding uses host OxideAV and the
pinned alloc-only `zenwebp 0.3.2` backend on freestanding targets. The shared
header parser and capacity checks cover VP8, VP8L, and VP8X/ALPH images; the
guest acceptance runs the signed WovenImages decoder in Ring 3. `zenwebp`
is AGPL-3.0-or-later, so production distribution requires license compliance
or a commercial replacement.

## Graphics roadmap Stage 06 iconography (2026-10-25)

`libwoven/src/icons.rs` owns the versioned code-native icon contract used by
WovenShell. It provides stable semantic IDs and MIME associations, bounded
8x8 masks rendered with integer scaling, and explicit theme/state colors. The
shell maps catalog entries to these IDs for launcher and desktop tiles. This
boundary deliberately excludes parsing untrusted SVG or fetching thumbnails;
those require a later capability-checked asset service.

## Graphics roadmap Stage 05 typography increment (2026-10-25)

`libwoven::graphics::FontFallback` is the scene-facing fallback provider for
validated TrueType faces. It keeps at most eight borrowed faces and a fixed
128-entry scalar-to-face/glyph/advance cache. The cache owns only validated
indices and metrics, so font storage remains owned by the VFS/font loader and
cannot be invalidated through cached pointers. Missing cmap or advance data
fails closed to the next face. This is an incremental boundary; complete
UAX #9/script shaping and full IME remain higher-level limits recorded in
`docs/graphics/stage-05-typography-evidence.md`; the bundled static-font
kernel loader/raster path is qualified by the Stage 05 QEMU matrix.
`libwoven::graphics::TextInputState` supplies the companion fixed 512-byte
UTF-8 editing state with scalar-boundary caret and selection operations.
`reorder_text_uax9` uses `unicode-bidi` hardcoded Unicode data for complete
paragraph visual reordering into caller-owned storage. `TextInputState` owns a
bounded IME preedit buffer and commits it transactionally into the main UTF-8
buffer, preserving selection state on overflow.
The licensed fixture at `assets/fonts/NotoSans-Regular.ttf` is validated for
cmap/metrics and loaded by the bounded kernel `ttf` module for simple-glyf
rasterization. Composite or malformed outlines fail closed to the bitmap
fallback.

## Software architecture mandate S00 inventory

The S00-S20 software-architecture execution track is mapped in
`docs/release-qualification/software-gap-inventory.md`. The inventory is a
classification of current source and retained evidence, not a second API or
roadmap. Native WovenHat interfaces remain the supported product contract;
Linux and Windows compatibility are separate opt-in programs. The inventory
must be refreshed at each accepted increment, and the machine-readable
checkpoint must continue to distinguish software qualification, native-device
qualification, trust/review prerequisites, and final production release.

The S01 ABI foundation lives in `libwoven/src/abi.rs`. `Contract::CURRENT`
identifies the supported native major, minor and descriptor versions;
`negotiate` accepts only the same major/descriptor and a client minor no newer
than the SDK. Unsupported requests return typed errors before any syscall or
IPC operation. The kernel syscall 243 copies fixed-size descriptors into
kernel-owned storage, negotiates through the same contract, and copies out
only a validated response. The independently built exit-status fixture
exercises current, unsupported-major and malformed-reserved-byte requests.
This is bounded native software qualification; it is not foreign-OS
compatibility or production trust evidence.

### Stage 28 - kernel-backed installation transaction

`kernel/src/installer.rs` is the privileged installation boundary reached by
the supervised `woven-installer` service. It validates the fixed-capacity plan,
unmounts the active volume before mutation, snapshots the running EFI artifact
through the kernel GPT/FAT reader, writes both GPT copies, formats the aligned
ESP and data partitions with the kernel FAT32 formatter, installs
`EFI/BOOT/BOOTX64.EFI`, and writes bounded account/settings provisioning files.
The syscall requires `StorageIo` plus the storage WovenGuard authorization;
the GUI never receives that authority. The `stage28-installer-test` feature
executes the complete transaction against a disposable RamDisk and verifies
the resulting EFI and settings files. The production policy is deliberately
single-target and fail-closed if no readable source EFI artifact exists.

### Stage 28 - WovenInstaller production system-app integration

The production `system-apps` and Stage 19 catalog-test kernel images embed the
signed package supplied through `WOVENHAT_INSTALLER_IMAGE`. The kernel verifies
the package with the same signed-app boundary as the existing ten entries and
publishes it read-only at `/bin/wovenhat-installer` with a catalog descriptor at
`/share/applications/woven-installer.app`. The catalog entry carries no
capability grant and is not privileged; it is only a bounded display adapter.
The system-app, release, interactive, desktop, shell, and catalog-test launchers
all map the eleventh manifest record to this environment variable. This is
catalog/image integration, not destructive installation or physical-media
support.

### Stage 28 - WovenInstaller package/catalog wiring

The existing `scripts/package-system-apps.py` release boundary includes the
freestanding `apps/woven-installer` ELF as a signed WAPP package and emits
`packages/woven-installer.wapp`, `catalog/woven-installer.app`, and a manifest
record with the stable `/bin/wovenhat-installer` install path. Its metadata
declares an empty capability set, `privileged: false`, and
`interface: display-only`; the packager rejects forbidden install, disk,
partition, format, and bootloader capabilities before publication. The
package metadata is deliberately authority-free. Kernel installation and image
embedding are documented in the production integration section above.

### Stage 28 - disposable UEFI boot artifact preparation and verifier

`scripts/prepare-uefi-boot-artifact.py` is a disjoint, file-image-only
boundary layered over the verified layout metadata. It requires one ESP and
writes only that region. The envelope stores canonical metadata for the fixed
UEFI fallback path `EFI/BOOT/BOOTX64.EFI`, the exact payload length, SHA-256,
ESP-relative payload bounds, and the artifact bytes. The verifier rechecks the
magic, JSON bounds, fixed path, size limit, payload containment, digest, and
zero-filled tail. A deterministic fixture is available for repeatable tests;
explicit source files must be regular, non-symlink files and are capped at
4 MiB.

The write transaction snapshots the entire ESP, fsyncs the new bytes, verifies
the result, and restores the snapshot for injected failures. This envelope is
not a FAT filesystem directory entry and does not invoke a mount, partitioner,
firmware API, or bootloader installer. It does not parse PE/COFF signatures,
validate Secure Boot signatures, hand off to firmware, or prove that the
WovenHat kernel boots. The result is deliberately marked
`bootability: not-qualified` until an independently generated artifact is
booted and evidenced in QEMU.

### Stage 28 - disposable file-image layout builder and verifier

`scripts/build-file-image-layout.py` is an isolated, test-only preparation
boundary. It accepts a validated `woveninstaller-plan-v1` document and an
already-created regular file only when `--test-harness-created` is explicitly
provided. Device, physical-drive, protected, symlink, missing, unaligned, or
out-of-bound targets fail closed.

The deterministic image layout reserves sector 0 for a protective MBR, sectors
1-33 and their backup for GPT-like headers/entries, aligns an 8 MiB ESP and
the plan's data regions to 1 MiB boundaries, and records canonical metadata
with a SHA-256 checksum. The verifier rechecks both GPT signatures, metadata,
partition bounds/alignment, and sorted-region overlap. It performs no device
I/O, partition API calls, mounts, formatting, bootloader writes, or hardware
access; it is not production installation.

### Stage 28 - bounded installer UI/state model

`scripts/installer-ui-model.py`
is a pure Python state machine for the future WovenInstaller presentation
layer. `InstallerState` contains only serializable screen, bounded summary,
progress, confirmation, and error fields; it owns no device, file, process, or
kernel handles. The legal flow is:

`welcome -> target-selection -> partition-review -> settings -> progress`

with cancellation from each active pre-terminal screen through
`cancelling -> cancelled`, and fail-closed error transitions to `failure`.
Success is legal only after explicit confirmation and 100 percent progress.
Terminal cancelled, success, and failure states reject all events, so retrying
requires a newly constructed state rather than an implicit continuation.

The model bounds ordinary summaries to 128 characters, error messages to 256,
progress messages to 128, rejects control characters, requires a caller to
assert that a selected target is safe and unambiguous, and requires a literal
`confirmed: true` event before progress can advance. This confirmation is an
admission boundary for a future destructive executor; the current model does
not execute or authorize any operation. It performs no disk discovery or
access, filesystem work, partitioning, formatting, bootloader installation,
subprocess invocation, graphical rendering, or installation.

### Stage 28 - immutable installer-plan validator

`scripts/validate-installer-plan.py` consumes only caller-supplied JSON. The
target must match exactly one inventory record by path and serial; system,
boot, removable, read-only, backup, external, and USB-labelled targets are
protected. Partition requests are bounded, sector-aligned, counted, and kept
below the target capacity with a fixed one-MiB reservation.

The validator normalizes a small explicit filesystem, user, timezone,
keyboard, and DHCP/static-network schema and renders sorted-key canonical JSON.
Its operation markers explicitly state that disk I/O, formatting,
partitioning, and installation are not requested or implemented. No device
path is opened and no host command is called.

### Stage 28 - read-only target-disk inventory and safety classification

`scripts/inventory-target-disks.py` is a pure parser/classifier for supplied
JSON output. It supports normalized records plus the common JSON shapes of
PowerShell `Get-Disk` and Linux `lsblk -J`; it never invokes either command.
The normalized record retains path/number, serial, capacity, transport,
removable, and system/boot flags. Classification protects removable and
backup-labelled candidates and exposes a system disk only when exactly one
record is marked system or boot.

Stable identity is mandatory: every record needs a path or number, a serial,
and a positive capacity, and serials must be unique. Any missing, duplicate,
or ambiguous value makes `safe_target_selection` false. This boundary is
read-only and informational; later installer work must add an independently
reviewed target-selection policy before any partitioning or write operation.

### Stage 28 - ISO capability probe (unvalidated)

`scripts/probe-installer-iso.py` is a read-only capability probe, not an ISO
builder. It validates only file-backed inputs: the source must have a
protective MBR and primary GPT header, a supplied EFI tree must contain a
non-empty `EFI/BOOT/BOOTX64.EFI`, and one of the supported ISO builders must be
discoverable in `PATH`. Missing prerequisites are reported as explicit
blockers and the process exits non-zero. The probe never mounts an image,
opens a physical disk, or writes an artifact. Even when prerequisites are
present it reports `ready-for-build-validation`, not support; support requires
booting the exact generated ISO in QEMU.

### Stage 28 - WovenInstaller bounded file-artifact boundary

`scripts/package-installer.py` consumes the existing `build.rs` UEFI disk
image and its release-validation SHA-256 record, or an explicitly named file
image, and writes only a new file-backed artifact directory. It validates
sector alignment and the protective-MBR/GPT signatures before copying. With
`--include-iso`, it invokes the reviewed file-only El Torito builder and adds
the exact ISO digest and metadata to `installer-manifest.json` and
`SHA256SUMS`. The release packager uses this same ISO path so the release
archive cannot silently omit the bootable ISO. Neither path discovers or
writes disks; physical-media installation remains a separate reviewed gate.

`scripts/test-uefi-iso-qemu.py` boots one exact ISO artifact with caller-
supplied OVMF and accepts the same 1/2/4 CPU matrix used by the catalog
gates. It preserves one serial log per run and fails if a required
firmware/kernel marker is absent or a failure marker appears.

### Stage 27.5 - WovenRecovery bounded environment

`kernel/src/recovery.rs` defines the fixed-size recovery ABI. Syscall 240 is
available only to a task holding `Capability::Recovery` and only while the
kernel recovery boot-mode gate is authorized. Status, filesystem-check, and
serial-log results are copied through caller-owned bounded buffers; malformed
lengths and unauthorized mode fail closed. Reset and reinstall currently
return an explicit admission-not-implemented value and have no storage side
effects.

`apps/woven-recovery` is packaged and signature-checked as a normal WAPP
system application. The QEMU recovery gate runs it in both unauthorized and
authorized modes. Its check command reports unavailable when no checkable
filesystem is mounted rather than fabricating success. The foundation does
not yet provide destructive reset/reinstall, installer orchestration, or
bootloader update handoff.

### Stage 26.5 - bounded internationalization foundation

`libwoven::i18n` is the bounded userspace foundation for locale-sensitive
presentation. `Locale` identifies language and region without heap storage;
`Catalog` stores at most 32 fixed-size key/value translations and replaces
entries atomically; `Date` validates Gregorian month lengths and formats the
supported locale order; `format_integer` writes into caller-owned storage; and
`validate_scalars` rejects surrogate and out-of-range Unicode scalar values.
`normalize_latin_nfc` adds a transactional, caller-buffered subset for common
Latin combining marks and rejects unsupported combining sequences.
`resolve_bidi_runs` provides caller-buffered strong-direction runs for common
Latin, Hebrew, Arabic, and numeric text; it deliberately does not claim the
full Unicode Bidirectional Algorithm.
The module intentionally does not claim complete normalization, full Unicode
property data, full UAX#9 bidi, shaping, keyboard layouts, IMEs, or
compositor/application
integration.

WovenShell keeps its English and Chichewa lock-screen catalogs in static
storage and fills them in place on first use; this avoids copying the several-
KiB catalog through the userspace stack. It is a bounded integration example,
not a claim that every desktop string is translated.

The bounded user stack is sixteen pages (64 KiB) plus a guard page. The
increase gives bounded codec, browser, compositor, and production-shell paths
room for nested work while retaining a protected guard page.

### Stage 24.59 â€” Change-PIN credential synchronization

The desktop Change-PIN path is a single kernel-authorized transaction across
the identity authority and `/etc/wovenhat/accounts`. `kernel::accounts::change_pin`
uses a kernel-generated bounded salt; the system-app identity runtime passes
that same salt to `change_pin_with_salt`, so the session credential and the
durable account record remain identical after rotation. If the durable rewrite
fails, the identity authority is restored to its prior credential. The
dedicated WovenShell acceptance gate covers the full UI and relock/unlock path
on 1/2/4 CPUs.

### Stage 24.58 - physical TPM broker boot activation

The system-app boot path now activates the physical broker only after loading
and revalidating both signed policy records, reading the configured attestation
public area, obtaining a fresh TPM caller nonce, and promoting the PCR-bound
credential root through the Stage 24.57 read boundary. Any missing record,
invalid metadata, TPM error, or cleanup failure leaves the broker absent while
the desktop continues. A live TPM is still required to qualify the success
branch.

### Stage 24.57 - physical TPM credential-root read boundary

`kernel/src/tpm2.rs` now provides the bounded physical read path that validates
the configured NV public area, starts and PCR-binds a managed policy session,
performs an authenticated read at offset zero, refreshes the TPM nonce, and
flushes the session on every exit. The 32-byte root remains kernel-local and is
wiped before the validated `CredentialRootLease` is returned. This boundary is
implemented and host/QEMU validated, but a real TPM execution is still needed
before release qualification.

### Stage 24.56 - signed TPM broker record generator

`tools/tpm-broker-config` is the release-side constructor for the fixed
`TCB1` policy record. It requires the dedicated TPM policy signing seed and
matching trusted public key in environment variables, validates production
bounds before signing, and emits exactly 182 bytes. It has no development-key
fallback and contains no TPM I/O or disk-writing path.

### Stage 24.55 - physical TPM readiness preflight

The read-only PowerShell preflight records host TPM visibility, readiness,
software-emulation availability, signed broker-record presence, and removable
disk identity. It explicitly reports raw-disk access, disk-state mutation,
media writes, and TPM provisioning as false; its result cannot be interpreted
as a hardware qualification pass.

### Stage 24.54 - kernel-derived TPM authorization secret

`CredentialRootLease` now retains its validated NV index and policy binding;
rotation must preserve both. Provisioned broker installation checks the signed
`TCB1` fields and live attestation Name, then derives the attestation object's
authorization value with a domain-separated root derivation. The value exists
only in bounded kernel memory and is wiped after broker construction. No
userspace or durable-file secret path is accepted.

### Stage 24.53 - signed TPM broker provisioning record

`TCB1` is a fixed-size Ed25519-signed WovenFS record containing only broker
policy: the reserved credential-root NV index, attestation handle, quote
scheme, PCR and expected digest, attestation Name, and NV policy. The parser
verifies the signature and all bounds before returning fields. Boot retention
revalidates the record and does not create a root or enable login; authorization
secrets are intentionally not persisted in this record.

### Stage 24.52 - physical TPM credential broker boundary

`HardwareCredentialBroker` owns the validated TPM descriptor, active
credential-root lease, signed attestation policy, attestation public key,
PCR policy, and signing authorization value. Its one-request path derives the
account challenge, starts a fresh policy session, applies `PolicyPCR`, issues
`TPM2_Quote`, verifies the returned quote through the existing Name/signature
boundary, flushes the session on success or failure, and moves the resulting
decision into the identity runtime. Syscall 234 requests the broker using the
kernel's owner-bound account name; syscall 233 consumes the one-shot result.
No broker is installed when physical provisioning inputs are absent.

### Stage 24.51 - identity hardware-decision consumption

The TPM broker moves a verified `CredentialDecision` into a one-entry
owner/session-bound pending slot. Syscall 233 consumes it only after exact
session ownership, active state, account-name context tag, and one-shot
hardware-generation checks. The decision is never copied to userspace; audit
action `IdentityHardware` records success/failure. The
`authorize_and_publish_hardware_credential` producer entry point is ready for
the physical TPM broker.

### Stage 24.50 - hardware credential decision

`authorize_hardware_credential` derives a domain-separated quote nonce from an
active `CredentialRootLease`, the lease generation, and a bounded identity
context. It then delegates to `authorize_quote`, preserving signed TAN1
attestation-Name binding, TPM ReadPublic Name binding, nonce/PCR checks, and
RSA/ECC verification. Only a `CredentialDecision` carrying the root generation
and verified attestation is returned; root material is never exposed. A
rotated or revoked lease cannot authorize a decision.

### Stage 24.49 - credential-root lifecycle

`CredentialRootLease` is a non-copyable kernel-owned lease around the
policy-validated 32-byte root. It carries a monotonic generation and active
state, wipes material on drop/revoke/rotation, rejects zero roots, and offers
only bounded HMAC derivation with a domain separator and generation. Rotation
is transactional: validation and generation allocation happen before the old
root is cleared; failed validation leaves the current lease unchanged.

### Stage 24.48 - TPM custody authorization

`PolicySessionCustodyAuthority` is a private-construction, generation-tagged
kernel token. Issuance validates the CRB control-area alignment/start method
and, on release system-apps boots, requires that the signed TAN1 policy was
retained successfully. Both TPM context persistence and recovery require this
token in addition to `AuthorizedWfsWriter`, preventing ordinary storage
authority from treating TPM context files as credential custody.

### Stage 24.47 - TPM policy-session nonce refresh

`TPM2_Quote` response decoding now validates the complete authorization area,
including bounded response nonce and HMAC lengths. `QuoteExecution` exposes
the response TPM nonce. Password-session quotes may return an empty nonce;
policy-session quotes require a non-empty refreshed nonce. The managed lease
copies that nonce before marking the quote one-shot stale, preserving the
updated session state for custody or cleanup without granting another quote.

### Stage 24.46 - boot-time TPM context recovery

System-apps boot captures the discovered TPM2 handle before the ACPI summary is
consumed. After native WovenFS publication and signed TAN1 policy loading, it
loads the newest validated `TPS1` context candidate, calls TPM2 `ContextLoad`,
and immediately calls `FlushContext`. This is deliberately a custody/recovery
probe only: until a credential-authority consumer is integrated, every path
keeps hardware credentials disabled. Storage-only records, missing TPMs,
malformed records, failed loads, and cleanup failures cannot create authority.

### Stage 24.45 - durable TPM context custody

The bounded `TPS1` record stores only TPM-produced context bytes plus
validated session metadata, in two journal-backed WovenFS paths with a
monotonic commit sequence and SHA-256 integrity digest. Recovery selects one
newest valid copy; ties and invalid copies fail closed. The record is not
authority: `TPM2_ContextLoad` must still accept it.

### Stage 24.44 - TPM policy-session context save/load

`save_policy_session` validates a bounded `TPMS_CONTEXT`, invalidates the
loaded lease only after successful `ContextSave`, and returns a one-use
snapshot. `load_policy_session` validates `ContextLoad`, consumes the
snapshot only after success, and issues a fresh generation with the restored
handle. Failed restore remains retryable; context storage is not yet a
cross-reboot credential store.

### Stage 24.43 - managed TPM policy-session lifecycle

`PolicySessionLease` owns one generation-tagged session handle and tracks
active/nonce-stale state. Managed PolicyPCR and quote operations require a
usable lease; a successful quote marks it stale, while `flush_policy_session`
performs `TPM2_FlushContext` and invalidates it only after exact success.
Cleanup failures preserve the active token for retry, preventing silent
stale-handle reuse.

### Stage 24.42 - policy-session quote HMAC derivation

`derive_quote_session_hmac` hashes the exact Quote command code, handle, and
parameters, then computes SHA-256 HMAC over cpHash, the session TPM nonce,
caller nonce, and attributes using the caller-supplied key auth value.
`quote_with_derived_policy_hmac` sends that result through the non-password
session quote boundary. This is intentionally scoped to null-salt/null-bind
sessions and does not read or invent key material.

### Stage 24.41 - live policy-session transcript execution

`start_policy_session` executes `TPM2_StartAuthSession` with bounded caller
nonce input and validates the returned session handle/TPM nonce.
`apply_policy_pcr` executes the bounded SHA-256 `TPM2_PolicyPCR` transcript
step and accepts only an exact successful response. These are state-changing
TPM operations, not credential authority; later work must refresh session
state and derive the quote authorization HMAC.

### Stage 24.40 - policy-session TPM2 Quote framing

`QuoteSessionAuth` carries a non-password session handle, caller nonce,
attributes, and bounded HMAC into the `TPM2_Quote` authorization area. The
policy-session API rejects the password-session handle, while the lower-level
encoder preserves explicit caller material and does not manufacture an HMAC.
Session creation, PolicyPCR, and cryptographic HMAC derivation remain
separate prerequisites.

### Stage 24.39 - bounded TPM2 Quote execution

`tpm2::quote` builds a sessions-tagged `TPM2_Quote` for one bounded nonce,
PCR selection, and explicit RSA/ECDSA scheme, executes it through the
serialized CRB transport, and returns bounded quote/signature buffers. The
wire result is evidence only; `authorize_quote` remains the required
identity, nonce, PCR, and signature-verification boundary. The current
transport uses an empty password session and therefore does not claim
non-empty policy-session authorization.

### Stage 24.38 - boot-time TPM Name policy loading

The system-apps boot path calls `tpm2::load_attestation_name_policy_at_boot`
after the durable WovenFS mount attempt and before WovenShell startup. The
loader uses `signed_app::TRUSTED_PUBLIC_KEY`, requires the exact TAN1 record,
and retains the verified bounded envelope in a kernel-owned slot for later
quote authorization. It treats storage absence or validation failure as
unavailable hardware credentials and does not synthesize a fallback identity
or block the desktop.

### Stage 24.37 - fail-closed TPM quote authorization integration

`authorize_quote` requires a signed TAN1 policy, a `ReadPublic` result whose
Name matches it, expected nonce/PCR evidence, a fully consumed TPM signature
frame, and the matching RSA or P-256 verifier. The structured request keeps
these inputs explicit and the function returns no authorization on any
mismatch. It is an authorization primitive, not yet a boot-time policy hook.

### Stage 24.36 - durable signed TPM Name policy boundary

`persist_attestation_name_policy` validates TAN1 before committing the exact
record through journal-backed `write_vfs_path_durable`; the read path accepts
only the exact bounded length and revalidates the Ed25519 signature. The
functions are guarded by the durable-storage/system-app feature set so lower
stages cannot accidentally acquire the dependency.

### Stage 24.35 - signed TPM attestation-Name policy

`verify_signed_attestation_name_policy` validates a fixed 106-byte TAN1
envelope, verifies its Ed25519 signature with the caller-provisioned trust
root, and returns only the signed SHA-256 TPM Name. It grants no authority
from storage presence; callers must still bind the result to `ReadPublic` and
then apply quote authorization.

### Stage 24.34 - TPM attestation-key Name binding

`decode_attestation_public` hashes the exact TPM public area and requires the
returned SHA-256 Name to match the recomputed 34-byte value. It retains the
bounded qualified Name, while `bind_attestation_public_name` compares that
identity with the provisioned expected Name before returning the key. This
does not itself provision or persist an expected identity.

### Stage 24.33 - bounded TPM ReadPublic attestation-key boundary

`encode_read_public` and `decode_attestation_public` implement the bounded
`TPM2_ReadPublic` command/response boundary. The decoder checks SHA-256 name
algorithm, fixed/restricted signing attributes, RSA modulus validity or P-256
point validity, exact TPM2B lengths, and the name/qualified-name tail. The
transport wrapper remains serialized and fail-closed; object-name binding is
still a higher-level responsibility.

### Stage 24.32 - bounded ECC/P-256 quote verification

`verify_quote_ecc_p256_sha256` builds an uncompressed SEC1 P-256 public key
from bounded TPM affine coordinates and verifies the exact quote bytes with
ECDSA/SHA-256. It requires canonical 32-byte `r` and `s` components and
rejects invalid points or signature mismatches. Key retrieval and TPM key
identity validation remain caller responsibilities.

### Stage 24.31 - bounded TPM RSA quote verification

`verify_quote_rsa_sha256` constructs a validated RSA public key from bounded
TPM modulus/exponent fields and verifies RSASSA or RSAPSS SHA-256 signatures
over the exact quote bytes. The TPM exponent value zero is normalized to
65537. This boundary authenticates only when its caller has independently
validated the TPM public key; it does not retrieve or provision that key.

### Stage 24.30 - bounded TPM quote signature framing

`decode_quote_signature` validates the bounded `TPMT_SIGNATURE` scheme and
SHA-256 hash selector, then parses exact-size RSA (RSASSA/RSAPSS) or ECDSA
components. It reports the consumed frame length but deliberately performs
no public-key or cryptographic verification; callers must not treat framing
as attestation.

### Stage 24.29 - bounded TPM quote attestation parsing

`decode_quote_attestation` validates the bounded `TPMS_ATTEST` quote shape,
generated-value magic, quote type, qualifying nonce, one SHA-256 PCR
selection, and exact 32-byte digest. It returns quote evidence but does not
authenticate the signing key or signature; those checks remain mandatory
before trust decisions.

### Stage 24.28 - credential-root read-back promotion boundary

`validate_credential_root_readback` first validates the complete NV public
metadata and then compares the expected and returned 32-byte root with a
constant-time loop. It returns no key and retains no state; callers may only
promote after this decision and independent authorization/attestation checks.

### Stage 24.27 - policy-session NV read-back

`encode_nv_read_policy` constructs a bounded policy-session `NV_Read` for the
fixed index, zero offset, and 32-byte value. The decoder validates the
sessions response tag, TPM status, parameter size, bounded value, and exact
response authorization nonce/HMAC area before copying the value to the
caller. It performs no live I/O or credential promotion.

### Stage 24.26 - bounded TPM NV definition framing

`encode_nv_define_space_policy` constructs a sessions-tagged,
owner-authorized `NV_DefineSpace` request for the fixed credential-root
metadata. It bounds the caller-supplied owner authorization, encodes the
32-byte SHA-256 policy and data size, and never persists the authorization or
performs TPM I/O. Physical provisioning remains a separate release gate.

### Stage 24.25 - policy-session NV write framing

`encode_nv_write_policy` constructs a bounded sessions-tagged `NV_Write` for
one 32-byte root payload, binding the auth handle and NV index and carrying a
policy-session authorization area with a bounded caller nonce. The response
decoder checks the sessions tag, TPM status, zero parameter area, bounded
nonce/HMAC, and exact framing. It performs no TPM I/O and does not expose a
persisted secret.

### Stage 24.24 - bounded TPM PolicyPCR transcript

`encode_policy_pcr` constructs a fixed SHA-256 `PolicyPCR` command for one
validated PCR and one active nonzero policy-session handle. The response
decoder accepts only a ten-byte successful TPM response. This establishes
the command transcript boundary but does not perform live TPM I/O or
authorize subsequent NV operations.

### Stage 24.23 - bounded TPM policy-session framing

`encode_policy_session_start` constructs a bounded no-session
`StartAuthSession` request for a policy session using null TPM/bind handles,
caller-supplied nonce, null symmetric protection, and SHA-256. The response
decoder requires a nonzero session handle and exact bounded TPM nonce. This
is transport framing only; it does not authorize an NV operation or expose a
credential root.

### Stage 24.22 - credential-root NV policy validation

`validate_credential_root_nv` checks an already-decoded NV public area against
the expected index and caller-provisioned 32-byte PCR policy. It requires a
32-byte SHA-256-named value, policy-authorized writes, and rejects owner or
object authorization writes and policy deletion. The function never performs
NV I/O and never exposes credential bytes; authorization sessions, object
creation, and policy-bound provisioning remain separate work.

### Stage 24.21 - bounded TPM NV public metadata

`tpm2::encode_nv_read_public` frames a no-session `NV_ReadPublic` request
and `decode_nv_read_public` accepts only an exact bounded response containing
the NV index, name algorithm, attributes, at most 32 policy bytes, data size,
and a bounded trailing Name field. It is metadata-only: no NV authorization,
write, secret read, sealing, or credential-root promotion is performed. A
live TPM transaction and policy-bound sealed provisioning remain future
trust-boundary work.

### Stage 24.20 - signed PCR policy boundary

`verify_signed_pcr_policy` accepts only a fixed `TPP1` envelope containing one
PCR index, one SHA-256 expected digest, and a signature over the complete
policy header/digest. The trusted key is caller-provisioned and never read
from the envelope. Signature failure, malformed bounds, and PCR mismatch are
fatal to the policy decision. The boundary does not provision a policy,
verify a TPM quote, or seal identity credentials.

### Stage 24.19 - bounded PCR evidence read

The TPM2 transport constructs a single SHA-256 `PCR_Read` selection and
accepts only one requested PCR, one SHA-256 selection, one 32-byte digest, and
an exact bounded response frame. Boot reads PCR 7 only for inventory and wipes
the digest after logging a short prefix. A PCR read is not an attestation:
credential decisions still require a provisioned expected policy or verified
TPM quote bound to the signed boot measurement.

### Stage 24.18 - bounded TPM2 CRB probe

`tpm2` serializes one polling CRB `GetRandom` transaction after validated ACPI
discovery. It validates the CRB interface type, fixed-size command and
response buffer capacities/addresses, fatal status, response code and length,
and uses a finite iteration timeout because early boot has no timer IRQs. The
probe output is wiped and is not a credential root. PCR/attestation validation
and sealed credential provisioning remain separate trust-boundary work.

### Stage 24.17 - TPM2 platform discovery boundary

`hal::acpi` validates and retains the bounded TPM2 ACPI descriptor
(`platform_class`, `control_area`, and `start_method`). Boot inventory reports
presence only; it never treats an address or table as authenticated hardware
evidence. Missing or malformed TPM2 data cannot silently downgrade the
identity or volume-crypto trust boundary.

### Stage 24.16 - durable/configurable lockout policy

`Authority` stores a bounded failed-authentication threshold (1..=32) in the
existing WID1 reserved header byte. Version 1 snapshots decode with the
default threshold for compatibility; version 2 snapshots persist the selected
value. Syscall 232 and audit action 37 update the policy only through an
owner-bound elevated administrator session. The runtime reads the authority
policy for plaintext and digest authentication, preserving the fixed
five-second cooldown and fail-closed lockout table. WovenSecurity exposes a
fixed-buffer decimal editor. Hardware-demo-only volatile persistence is
explicitly excluded from production behavior.

### Stage 24.15 - administration mutation UI

WovenSecurity drives all existing bounded identity mutations from one
owner-bound elevated session: user/group creation, supplementary membership
add/remove, and inactive-user deletion. Prompts use fixed buffers and decimal
ID parsing; successful operations refresh the query directory. The identity
runtime, rather than the desktop-only `AccountManage` capability, authorizes
these mutations because the signed security application is a normal userspace
process. Runtime owner/elevation checks remain mandatory for every operation.
The `hardware-demo` feature permits volatile acceptance when no WovenFS volume
exists; production publication remains durable and fail-closed.

### Stage 24.14 - bounded administration query UI

`libwoven::identity::AdminQuery` decodes a fixed version/kind/count wire
format for bounded user and group records. The kernel exposes syscall 231
through the owner-bound identity runtime; it requires an elevated
administrator session and copies only bounded non-secret records to userspace.
WovenSecurity owns the session and clears its PIN buffer on close or failure,
then renders a fixed-size directory. The query is deliberately separate from
the mutation capability gate because the signed WovenSecurity process uses the
identity elevation proof as its authorization boundary; mutation syscalls keep
their existing capability checks.

### Stage 24.13 - group membership administration

`Authority` exposes bounded add/remove operations for supplementary group membership. The owner-bound runtime authorizes syscalls 229-230, persists the updated WID2 authority, and restores the prior live authority if publication fails. Primary-group removal is prohibited; audit actions distinguish the two mutations. UI/query integration and physical qualification remain separate.

### Stage 24.12 â€” durable identity administration

Admin mutations publish the new authority through WID2 and synchronize the
trusted account file. The runtime retains a copy of the previous authority
and attempts WID2 rollback if file publication fails. The SMP migration test
waits for coverage-probe task reclamation before allocating stress slots.

### Stage 24.11 â€” bounded identity administration ABI

Syscalls 226â€“228 expose add-user, delete-user, and add-group operations to
the trusted elevated administrator session. Requests are copied into bounded
buffers; the policy rejects ordinary callers, self-deletion, and deletion of
accounts with active sessions. Actions are recorded in the security ledger.
The current slice changes the live authority only; durable account/WID2
publication and UI integration remain separate work.

### Stage 24.10 â€” kernel identity lockout/rate limiting

`identity_runtime` keeps a fixed per-account failure table. Five failed
password or verified-digest attempts impose a five-second tick-based cooldown;
successful authentication clears the entry. The table is reset when a new
authority is installed and fails closed if capacity is exhausted. WovenShell
derives a candidate digest for each PIN entry so the kernel sees failed login
manager attempts.

### Stage 24.9 â€” live PIN rotation consistency

The account-change syscall updates the owner-bound live `Authority` and the
trusted account file together. It generates a fresh salt, verifies the old
credential, and rolls the live authority back if the file rewrite fails.
WovenShell reloads its bounded account cache after a successful change.

### Stage 24.8 â€” WID2 runtime recovery

System-apps startup calls the native `identity_storage::load` adapter before
WovenShell starts. The newest checksum-valid WID2 copy is decoded into a fresh
`libwoven::identity::Authority`; invalid or missing durable records fail closed
to the bounded account-file bootstrap. No session state is persisted or
restored by this path.

### Stage 24.7 â€” WovenIdentity audit actions

Identity syscalls record valid open, lock, unlock, elevation, and close
outcomes through `kernel::audit`. `libwoven::audit_read` maps the stable
action IDs 26â€“30 to the public security ABI. The ledger records the calling
task, session target where applicable, and whether the operation was allowed.
Malformed buffer handling remains fail-closed before an operation is formed;
separate malformed-request telemetry is still a follow-up.

### Stage 24.6 â€” live WovenIdentity runtime and WovenShell login

`kernel/src/identity_runtime.rs` loads bounded account-file records into a
fresh authority before the production session is spawned. WovenShell owns the
interactive PIN verification; its `AccountManage` capability gates the
verified-digest open/unlock syscalls 224â€“225, and the kernel compares the
digest against its own stored credential before applying the owner-bound
process UID/GID. `task::exit_current_process` releases all bindings owned by
the exiting process before scheduler teardown. Authenticated WID2 recovery,
lockout, auditing, and PIN rotation remain separate prerequisites.

### Stage 24.5 â€” WovenIdentity syscall ABI

Syscalls 219â€“223 are additive identity calls: open, lock, unlock, elevate, and
close. Open carries `[name_len][name][password_len][password]`; password
actions carry `[length][password]`. Kernel handlers reject bounded-buffer
violations before calling `identity_runtime`, and the `libwoven::identity`
wrappers construct exact-length stack buffers. The ABI does not retain user
pointers. Runtime snapshot loading, live Ring-3 exercise, teardown reclamation,
audit, lockout, and shell migration are intentionally later acceptance slices.

### Stage 24.4 â€” owner-bound kernel identity seam

`kernel/src/identity_runtime.rs` keeps fixed session bindings `(SessionId,
process_id)` separate from `PROCESS_TABLE`. It validates ownership before every
state change and releases the identity lock before calling the crate-only
`task::set_current_credentials` setter. A failed application closes the
authority session and removes its binding. This stage preserves the existing
`getuid/getgid` syscall ABI but intentionally does not expose new syscalls;
the next slice must add bounded user-pointer framing, durable snapshot loading,
exit/reap cleanup, audit actions, and WovenShell integration.

### Stage 24.3 â€” kernel-authorized identity storage

`kernel/src/identity_storage.rs` is the narrow storage adapter. It accepts the
kernel-only `AuthorizedWfsWriter`, validates the `WID1` snapshot, and writes a
fixed `WID2` wrapper to `/wfs/identity-state-a` or `-b`. The wrapper carries a
non-zero generation and checksum; recovery ignores missing/corrupt copies and
chooses the newest valid one, while equal generations fail closed. The QEMU
gate uses a disposable IDE image and leaves external media untouched. This
adapter does not serialize sessions or perform task credential binding.

### Stage 24.2 â€” WovenIdentity authenticated snapshot

`Authority::encode_snapshot` and `decode_snapshot` use the fixed-size `WID1`
format defined by `libwoven::identity::SNAPSHOT_BYTES`. Records include only
identity policy state: users, groups, administrator roles, profile display
names, salts, and password digests. A checksum covers the complete padded
record; decoding validates counts, names, IDs, memberships, padding, and
duplicates before publishing a fresh authority. Sessions are deliberately
absent from the format. The kernel must still provide authorized atomic
filesystem persistence, recovery/key management, rotation, and task binding.

### Stage 24.1 â€” WovenIdentity policy core

`libwoven/src/identity.rs` is the allocation-free local identity policy core.
`Authority` owns fixed-capacity users, groups, and sessions; `Credential`
stores only a PBKDF2-HMAC-SHA256 digest and salt; sessions explicitly track
active/locked state and elevation. Lock always clears elevation, unlock and
elevation require the account password, and names/profiles/capacities are
validated before state mutation. This module is intentionally not a kernel
credential switch or durable database. The next identity stage must integrate
it with generation-safe task credentials, authenticated durable storage,
capability authorization, lockout/rate limiting, and the login UI.

## Persistent session account import (2026-10-20)

The hardware-demo/session boot path checks the read-only mounted FAT32 view at
`/mnt/etc/wovenhat/accounts` before starting WovenShell. When present, the
bounded salted-hash account file is read into a heap buffer, minimally
validated (`name:salt-hex:hash-hex`, with the shared size limits), and copied
read-only to `/etc/wovenhat/accounts`. A malformed, oversized, or non-file
source fails closed; a missing source permits only the explicitly disposable
QEMU demo-account fallback. This keeps persistent account installation ahead
of session creation without storing PINs or making the shell trust malformed
input.

## Session track - packaged app launch regression acceptance (2026-10-20)

After the SMP sign-in repaint fix, the production WovenCalculator package was
launched through WovenShell and its bounded pointer control was exercised on
QEMU 1/2/4 CPUs with changed-frame evidence. The remaining packaged clients
retain their earlier 1/2/4-CPU acceptance; the shell change is confined to
lock-screen damage selection. Persistent account provisioning remains open.

## Session graphics - bounded SMP sign-in repaint (2026-10-20)

WovenShell's lock/sign-in key path returns bounded damage for the changed
account/PIN region. Account selection repaints the account rows plus prompt;
digit, backspace, and wrong-PIN states repaint only the prompt/dot region; a
successful sign-in returns the full desktop area. This keeps multicore input
acceptance from waiting on a full-screen wallpaper redraw and preserves the
same compositor ownership and input-focus rules. QEMU Stage 20.12 and 20.15
pass on 1/2/4 CPUs. The interactive hardware-demo accounts remain volatile;
persistent release account provisioning is separate.

## Graphics track - WovenGraphics compositor visual acceptance (2026-10-19)

Stage 19.2 was revalidated on QEMU 1/2/4 CPUs. The `hello-gui` frame passed
the WovenGraphics visual, 2D, and software 3D markers after compositor
presentation, and the same runs verified retained-window behavior, clean
application exit, and owner-slot release. This is software rendering through
the production compositor; it does not claim accelerated GPU scanout.

## Graphics track - standard virtio-gpu 2D scanout acceptance (2026-10-19)

The standard `virtio-gpu-pci` backend remains the authoritative visible
presentation path. Fresh QEMU 1/2/4-CPU runs captured both 640x400 heads and
verified every pixel against the gradient/checkerboard contract. The driver
owns bounded per-head 2D resources, transfers changed rows, flushes the
resource, and presents it through the existing scanout path. This accepted
software/2D path is intentionally separate from the optional GL/VirGL
transport and its unsupported visible-scanout boundary.

## Graphics track - VirGL 2D-on-GL scanout boundary (2026-10-19)

The stage 20.14 VirGL probe now separates lifecycle acceptance from visible
scanout. After capability/context/resource/command validation, it attempts to
bind a conventional 2D resource, fill the known gradient, and present it.
QEMU rejects `SET_SCANOUT` with `Device(4611)` (`0x1203`) on the tested
GL-capable device. The probe therefore logs an unavailable capability and
keeps the lifecycle gate passing; it does not claim a GPU-backed frame. The
software/2D virtio-gpu path remains the authoritative fallback until a visible
GPU frame is accepted on a supported device.

## Graphics track completion boundary (2026-10-19)

The ten packaged desktop clients now each have a QEMU-accepted bounded
functional pointer control, with 1/2/4-CPU visual evidence and host checks
recorded in the audit documents. The software WovenGraphics/Canvas path,
compositor, shell interaction, client resize, pointer forwarding, and client
control loop are therefore integrated and visually accepted in QEMU.

The remaining hardware-dependent boundary is GPU acceleration: the VirGL
1/2/4-CPU gate proves capability/context/resource/command transport lifecycle,
but the tested QEMU 3D/blob resource is not accepted for visible scanout.
Software/2D presentation remains the authoritative fallback until a visible
GPU-backed frame is accepted; physical GPU and USB HID testing are separate.

## Graphics WovenUpdate and WovenStore pointer controls (2026-10-19)

WovenUpdate renders a bounded `Refresh` control at local x=250..329,
y=145..164 in its 360x200 preflight surface. WovenStore renders one at
x=250..329, y=170..189 in its 360x220 catalog surface. A primary-button
release forwarded through `WPBT` re-reads each bounded read-only view,
increments a saturating refresh counter, and repaints. Neither control
installs packages or mutates the catalog. Both QEMU gates passed on 1/2/4
CPUs. All ten packaged clients now have accepted functional pointer controls;
USB HID and hardware GPU qualification remain separate.

## Graphics WovenImages pointer control (2026-10-19)

WovenImages renders a bounded `Refresh` button in its 360x220 surface at
local x=250..329 and y=170..189. On a primary-button release in that
rectangle, it re-reads the bounded `/mnt` image summary, increments a
saturating refresh counter, and repaints. The desktop control remains
read-only; image decoding and mutation are not enabled. QEMU visual
acceptance passed on 1/2/4 CPUs; USB HID and hardware GPU qualification remain
separate.

## Graphics WovenEditor pointer control (2026-10-19)

WovenEditor renders a bounded `Refresh` button in its 360x220 surface at local
x=250..329 and y=170..189. On a primary-button release in that rectangle, it
re-reads the bounded `/mnt` workspace summary, increments a saturating refresh
counter, and repaints. The desktop control remains read-only; the existing
CLI write path is unchanged. QEMU visual acceptance passed on 1/2/4 CPUs;
USB HID and hardware GPU qualification remain separate.

## Graphics WovenSecurity pointer control (2026-10-19)

WovenSecurity renders a bounded `Refresh` button in its 360x200 surface at
local x=250..329 and y=145..164. On a primary-button release in that
rectangle, it re-reads the bounded audit summary, increments a saturating
refresh counter, and repaints. The control is read-only and does not mutate
audit state. QEMU visual acceptance passed on 1/2/4 CPUs; USB HID and
hardware GPU qualification remain separate.

## Graphics WovenMonitor pointer control (2026-10-19)

WovenMonitor renders a bounded `Refresh` button in its 360x220 surface at
local x=250..329 and y=170..189. On a primary-button release in that
rectangle, it re-reads its clock/display/storage snapshot, increments a
saturating refresh counter, and repaints. The control is read-only. QEMU
visual acceptance passed on 1/2/4 CPUs; USB HID and hardware GPU qualification
remain separate.

## Graphics WovenTerminal pointer control (2026-10-19)

WovenTerminal renders a bounded `Clear` button in its 360x220 preview surface
at local x=250..329 and y=170..189. On a primary-button release in that
rectangle, it clears only the bounded GUI command preview and replaces the
status text; the full CLI/interactive path is unchanged. QEMU visual
acceptance passed on 1/2/4 CPUs. USB HID and hardware GPU qualification remain
separate boundaries.

## Graphics WovenFiles pointer control (2026-10-19)

WovenFiles renders a bounded `Refresh` button in its 320x240 client surface at
local x=220..299 and y=206..225. On a primary-button release in that rectangle,
the client re-reads the bounded `/mnt` directory listing, increments a
saturating refresh counter, and repaints. The control is read-only and does
not mutate storage. QEMU visual acceptance passed on 1/2/4 CPUs. USB HID and
hardware GPU qualification remain separate boundaries.

## Graphics Settings pointer control (2026-10-19)

WovenSettings renders a bounded `Refresh` button in its 360x240 client
surface at local x=250..339 and y=190..213. On a primary-button release in
that rectangle, the client increments a bounded refresh counter, re-renders
the current identity/clock/display values, and emits a delivery marker. The
operation is read-only and non-persistent. QEMU visual acceptance passed on
1/2/4 CPUs. This extends functional controls to Settings while USB HID and
hardware GPU qualification remain separate.

## Graphics Calculator pointer control (2026-10-19)

WovenCalculator renders a bounded `Evaluate` button in its 240x150 client
surface at local x=150..209 and y=110..127. WovenShell forwards a primary
button release as `WPBT` with clamped client-local coordinates; the calculator
checks that rectangle, evaluates its bounded expression, and repaints the
result. The QEMU visual gate passed on 1/2/4 CPUs. This is the first accepted
functional client control; the other packaged clients currently provide
pointer markers only, and USB HID/hardware GPU qualification remains outside
this gate.

## Graphics client pointer forwarding (2026-10-19)

WovenShell owns desktop chrome, title-bar drags, close controls, and pointer
capture. A pointer move over the topmost client body produces a bounded
`WPMV` message containing client-local `u16` x/y coordinates. A primary-button
press/release captured by that client produces `WPBT` with button, edge, and
client-local coordinates; values are clamped to the shared surface. The shell
does not forward events while dragging a title bar.

All ten packaged clients consume both messages and repaint bounded pointer
markers. The QEMU visual gate passed for each client on 1/2/4 CPUs and checks
both the delivery marker and a presented-surface pixel delta. This does not
imply that the clients have full functional controls, that USB HID is
qualified, or that presentation uses a hardware GPU.

## Graphics window interaction acceptance (2026-10-19)

Stage 20.4 is the authoritative QEMU acceptance for production WovenShell
window movement and closure. On 1/2/4 CPUs, its real i8042 input path drags
Alpha by the title bar, verifies repaint at the old and new locations, closes
the larger Wide client through its close control, routes focus and a key to
Alpha, and verifies that the shell returns the display on teardown. The
evidence is under `tar\target\woven-shell-windows-{1,2,4}cpu`.

The original Stage 20.4 gate did not cover client pointer forwarding; the
newer Calculator protocol section above covers that path. USB HID
qualification, hardware GPU presentation, and direct QMP automation of a
production WovenFiles drag remain separate boundaries.

## Graphics desktop pointer visual acceptance (2026-10-19)

The production desktop QEMU harness now has `--exercise-pointer`. It injects
bounded relative motion and a left click through QEMU's input path, then
requires a stable before/after screendump with a material cursor/frame delta.
The gate passed on 1/2/4 CPUs. This qualifies the PS/2-backed software cursor
overlay and existing shell hit-testing; USB HID pointer qualification and
hardware GPU presentation remain separate boundaries.

## Graphics WovenFiles bounded resize (2026-10-19)

`apps/woven-files/src/main.rs` now handles `WSIZ` by creating and sharing a
replacement surface, publishing `WRSZ` before the bounded Canvas upload, and
clamping requests to a loader-safe 480Ã—320 static render target. The QEMU
resize gate passed on 1/2/4 CPUs. The `/mnt` listing remains read-only and
bounded; file-operation input, pointer widgets, and hardware GPU output are
separate boundaries.

## Graphics WovenStore bounded resize (2026-10-19)

`apps/woven-store/src/gui.rs` now handles `WSIZ` by creating and sharing a
replacement surface, publishing `WRSZ` before the bounded Canvas upload, and
clamping requests to a loader-safe 480Ã—320 static render target. The QEMU
resize gate passed on 1/2/4 CPUs. Store remains a read-only catalog; package
mutation, pointer widgets, and hardware GPU output remain separate boundaries.

## Graphics WovenUpdate bounded resize (2026-10-19)

`apps/woven-update/src/gui.rs` now handles `WSIZ` by creating and sharing a
replacement surface, publishing `WRSZ` before the bounded Canvas upload, and
clamping requests to a loader-safe 480Ã—320 static render target. The QEMU
resize gate passed on 1/2/4 CPUs. Update remains preflight-only and
non-mutating; pointer widgets and hardware GPU output remain separate
boundaries.

## Graphics WovenImages bounded resize (2026-10-19)

`apps/woven-images/src/gui.rs` now handles `WSIZ` by creating and sharing a
replacement surface, publishing `WRSZ` before the bounded Canvas upload, and
clamping requests to a loader-safe 480Ã—320 static render target. The QEMU
resize gate passed on 1/2/4 CPUs. Image decoding/editing, pointer widgets, and
hardware GPU output remain separate boundaries.

## Graphics WovenEditor bounded resize (2026-10-19)

`apps/woven-editor/src/gui.rs` now handles `WSIZ` by creating and sharing a
replacement surface, publishing `WRSZ` before the bounded Canvas upload, and
clamping requests to a loader-safe 480Ã—320 static render target. The QEMU
resize gate passed on 1/2/4 CPUs. The desktop client remains read-only; CLI
editing, pointer widgets, and hardware GPU output are separate boundaries.

## Graphics WovenSecurity bounded resize (2026-10-19)

`apps/woven-security/src/gui.rs` now handles `WSIZ` by creating and sharing a
replacement surface, publishing `WRSZ` before the bounded Canvas upload, and
clamping requests to a loader-safe 480Ã—320 static render target. The QMP
resize gate passed on 1/2/4 CPUs. Security remains read-only with audit output
capped at 16 events; controls, pointer widgets, and hardware GPU output are
not implied.

## Graphics WovenMonitor bounded resize (2026-10-19)

`apps/woven-monitor/src/gui.rs` now negotiates `WSIZ/WRSZ` replacement
surfaces with a loader-safe 480Ã—320 static Canvas cap and redraws its bounded
telemetry snapshot after resizing. The QMP resize gate passed on 1/2/4 CPUs.
Monitor remains read-only; pointer controls and hardware GPU output are not
implied.

## Graphics WovenSettings bounded resize (2026-10-19)

`apps/woven-settings/src/gui.rs` now handles `WSIZ` with a loader-safe static
480Ã—320 Canvas target, publishes a shared replacement surface with `WRSZ`, and
redraws after the resize. The QMP resize gate passed on 1/2/4 CPUs. Settings
remains read-only; pointer controls and hardware GPU output are not implied.

## Graphics bounded WSIZ/WRSZ resize negotiation (2026-10-19)

WovenShell allows five seconds for a client replacement surface to arrive.
WovenCalculator and WovenTerminal handle `WSIZ`, clamp each requested
dimension to a 480Ã—320 static render target, share the new surface, and send
`WRSZ` before the potentially slow Canvas upload. The QMP resize gate passed
on 1/2/4 CPUs. The cap is a deliberate Ring-3 image/stack safety boundary;
other clients and hardware GPU presentation remain separate work.

## Graphics client damage registration and Calculator input (2026-10-19)

`kernel/src/user_graphics.rs` records damage for userspace surface writes,
fills, strokes, and text mutations, waking a shared compositor client as
needed. `apps/woven-calculator/src/main.rs` consumes bounded ASCII/WKEY input
and redraws its expression/result surface. The QMP interaction gate passed on
1/2/4 CPUs. Pointer widgets, WSIZ resize, and hardware GPU output remain
separate boundaries.

## Graphics bounded key protocol and Terminal input (2026-10-19)

WovenShell keeps ASCII text as one-byte owner messages and forwards Enter and
Backspace as `WKEY` plus a bounded key code to the focused owner. The Terminal
GUI consumes those messages and redraws its shared Canvas surface after
bounded edits; the QMP gate verifies pre/post input pixels on 1/2/4 CPUs.
Desktop command execution, pointer widgets, WSIZ resize, and hardware GPU
output remain separate work.

## Graphics WovenCalculator desktop acceptance (2026-10-19)

The existing `apps/woven-calculator/src/main.rs` Canvas/WOPN client is covered
by the shared QMP visual gate on 1/2/4 CPUs, using the production `--gui %p`
catalog entry. The preview evaluates a fixed `2 + 2` expression; CLI
expression evaluation remains authoritative, and desktop input, WSIZ resize,
and hardware GPU output are not implied.

## Graphics WovenTerminal desktop preview (2026-10-19)

`apps/woven-terminal/src/gui.rs` adds a bounded read-only terminal preview
using a static Canvas buffer and the WovenShell WOPN/WCLS lifecycle; the
production catalog supplies `--gui %p`. The shared QMP visual gate passed on
1/2/4 CPUs. CLI `--interactive` remains the input/command path; desktop key
forwarding, command execution, WSIZ resize, and hardware GPU output are not
implied.

## Graphics WovenUpdate/WovenStore clients and catalog capacity (2026-10-19)

`apps/woven-update/src/gui.rs` and `apps/woven-store/src/gui.rs` add bounded
read-only preflight/catalog windows using the WovenShell WOPN/WCLS lifecycle;
their production catalog entries pass `--gui %p`. The shared QMP visual gate
passed both clients on 1/2/4 CPUs. WovenShell `MAX_APPS` is 10, matching the
ten signed production descriptors so alphabetical loading does not drop
Update or Terminal. Update performs no installation and Store performs no
mutation; no controls, pointer widgets, or hardware GPU output is implied.
Bounded WSIZ/WRSZ for both clients is covered by their newer resize sections.

## Graphics WovenImages client (2026-10-19)

`apps/woven-images/src/gui.rs` adds a bounded image-workspace preview using a
static Canvas buffer and the WovenShell WOPN/WCLS lifecycle; the catalog
supplies `--gui %p`. The shared QMP launcher/screendump gate passed on 1/2/4
CPUs. Image decoding and input-driven selection are not implemented; the
existing bounded `--list`/`--info` CLI remains authoritative. Bounded
WSIZ/WRSZ is covered by the newer Images resize section.

## Graphics WovenEditor client (2026-10-19)

`apps/woven-editor/src/gui.rs` adds a bounded read-only workspace preview
window. It uses a static Canvas buffer and the WovenShell WOPN/WCLS lifecycle;
the catalog supplies `--gui %p`. The shared QMP launcher/screendump gate
passed on 1/2/4 CPUs. CLI `--view`/`--write` remains the editing interface;
desktop text input, pointer widgets, and hardware GPU output are not implied.
Bounded WSIZ/WRSZ is covered by the newer Editor resize section.

## Graphics WovenSecurity client (2026-10-19)

`apps/woven-security/src/gui.rs` is a bounded read-only desktop client for
identity and audit summary data. It uses a static Canvas buffer and the
WovenShell WOPN/WCLS lifecycle; the catalog supplies `--gui %p`. The shared
QMP launcher/screendump gate passed on 1/2/4 CPUs. Audit display is capped at
16 events, with no controls, pointer widgets, or hardware GPU output implied;
bounded WSIZ/WRSZ is covered by the newer Security resize section.

## Graphics WovenMonitor client (2026-10-19)

`apps/woven-monitor/src/gui.rs` is a bounded read-only desktop client for
clock, display geometry, and storage-count telemetry. It uses a static Canvas
buffer and the WovenShell WOPN/WCLS lifecycle; the catalog supplies
`--gui %p`. The shared QMP launcher/screendump gate passed on 1/2/4 CPUs.
Storage enumeration remains capped at 128 entries, and no controls, pointer
widgets, or hardware GPU output is implied; bounded WSIZ/WRSZ is covered by
the newer Monitor resize section.

## Graphics WovenSettings client (2026-10-19)

`apps/woven-settings/src/gui.rs` is a bounded, read-only desktop client. It
uses a static full-window Canvas buffer, displays identity/UTC/display data,
shares its `GraphicsSurface` with WovenShell, and follows the same WOPN/WCLS
lifecycle as the other clients. The catalog supplies `--gui %p`. The shared
QMP visual harness accepts `--app settings` and passed on 1/2/4 CPUs. No
interactive controls, pointer widgets, or hardware GPU output is implied;
bounded WSIZ/WRSZ handling is covered by the newer Settings resize section.

## Graphics WovenFiles QEMU visual acceptance (2026-10-19)

`scripts/test-files-desktop-qemu.py` is the live client-to-desktop gate. It
packages the signed system applications, boots the hardware-demo session,
drives sign-in and the F1 launcher through QMP, then captures the desktop
before and after WovenFiles opens. It requires stable capture geometry and a
substantial pixel delta in addition to the WOPN/window-open serial markers.
The gate passed on 1/2/4 CPUs. This proves software/2D desktop presentation,
not hardware GPU acceleration; Files still has no file-operation UI, and its
bounded WSIZ/WRSZ path is covered by the newer Files resize section. Pointer
visual acceptance is covered by the newer desktop pointer section.

## Graphics WovenFiles client update (2026-10-19)

`apps/woven-files` now follows the ordinary desktop-client boundary: its
`--gui <shell-pid>` path uses a static bounded Canvas pixel buffer, renders a
`/mnt` directory listing, shares the surface, sends `WOPN`, and blocks on the
shell-owned close message. The production catalog supplies `--gui %p`. App
Clippy and the 1/2/4-CPU system-app packaging/boot gates qualify the client
boundary; the live launcher/screenshot acceptance passed on 1/2/4 CPUs and
bounded WSIZ/WRSZ is covered by the newer Files resize section.

## Graphics GSUB multiple-substitution extension evidence (2026-10-19)

The type-2 multiple-substitution implementation is regression-tested through
both a direct lookup and a type-7 extension wrapper. A sequence count above
the 16-glyph bound fails closed before output generation. The acceptance still
covers software shaping only; full feature ordering and hardware GPU output
remain outside this boundary.

## Graphics GSUB multiple-substitution update (2026-10-19)

The shaping pipeline now supports bounded GSUB lookup type 2 sequences. A
checked coverage/sequence record can emit up to 16 replacement glyph IDs, and
the caller's shaped-output capacity remains the final bound. Direct and
type-7 extension-wrapped lookups share the parser; the first emitted glyph
retains the source character/count and later glyphs are continuation records.
The variation-aware shaping path uses the same bounded expansion. This does
not claim complete OpenType feature ordering or hardware GPU acceleration.

## Graphics VIRGL lifecycle evidence update (2026-10-19)

The capability-gated `virtio-gpu-gl-pci` probe passes on 1/2/4 CPUs for
capability discovery, context/resource creation and teardown, and bounded
VIRGL command submission. This is transport/lifecycle evidence only. The
tested 3D resource is still not accepted by QEMU's classic `SET_SCANOUT`, and
the experimental blob resource is not accepted by the VirGL context; the
software/2D scanout remains the accepted fallback until a visible GPU frame is
verified.

## Graphics GSUB reverse-chaining update (2026-10-19)

The bounded GSUB pipeline supports lookup type 8 reverse-chaining
substitutions. It checks the current coverage, nearest-first backtrack and
forward lookahead coverages, then selects the corresponding replacement glyph;
type-7 extension wrappers share the same parser. Sequence counts and table
extents are capped, and malformed data fails closed. This remains software
shaping/rasterization; hardware GPU acceleration is not implied.

## Graphics GSUB alternate update (2026-10-19)

The bounded GSUB pipeline supports lookup type 3 alternate substitutions by
selecting the first checked alternate, including type-7 extension wrappers;
malformed coverage and alternate sets fail closed.

## Graphics shared-font metric update (2026-10-19)

The shared kernel fontâ€™s `text_width` and `line_count` use saturating
accumulation, so WovenGraphics measurement remains defined for arbitrarily
long UTF-8 input before layout applies its own bounded drawing rules.

## Graphics surface dimension update (2026-10-19)

Canvas and DepthBuffer reject non-empty dimensions above `i32::MAX`, matching
the coordinate type used by their internal pixel-offset conversions in
addition to checking the far edge.

## Graphics Canvas text-origin update (2026-10-19)

`Canvas::draw_text` saturates cursor movement, glyph origins, and newline
placement before calling the Canvas clip boundary, so the low-level text API
shares the rendererâ€™s overflow behavior.

## Graphics surface-coordinate invariant (2026-10-19)

Canvas and DepthBuffer accept only non-empty rectangles whose right and bottom
coordinates fit the `i32` APIs used by shader and raster offset arithmetic.
This makes the downstream coordinate additions safe by construction.

## Graphics affine transform update (2026-10-19)

`Transform::scale` and `translate` saturate fixed-point construction, while
`Transform::map` uses `i128` products and clamps mapped coordinates to `i32`.
This keeps transform inputs safe before the bounded triangle path.

## Graphics path-fill arithmetic update (2026-10-19)

Path scan conversion and supersampled compound containment use `i128`
intersections for `i32` scene coordinates. Canvas clipping remains the work
bound, while edge arithmetic no longer overflows before the clip.

## Graphics path stroke update (2026-10-19)

`Renderer::stroke_line` clips each segment to the Canvas with widened integer
intersections before running Bresenham. This preserves visible line coverage
while bounding work by the target surface rather than endpoint distance.

## Graphics path-bound update (2026-10-19)

Rounded-rectangle and path scan bounds use the shared `rect_end` saturating
helper, while radius conversion is checked. This keeps Canvas clipping stable
for extreme dimensions without changing normal path coverage behavior.

## Graphics text-layout update (2026-10-19)

`Renderer::text_centered_scaled` and `text_box` use checked rectangle offsets
and saturated edges; `text_scaled` saturates cursor and glyph-origin
arithmetic before delegating to clipped fills. This keeps layout deterministic
without allowing extreme caller rectangles to wrap coordinates.

## Graphics transformed-rectangle corner update (2026-10-19)

`Renderer::transformed_rect` derives its exclusive far edges with checked
conversion and saturation before calling the fixed-point transform. This
preserves the clipped triangle path while preventing dimension narrowing from
wrapping geometry.

## Graphics image blit update (2026-10-19)

`Renderer::image` and `image_blended` clip their iteration to the Canvas and
derive source offsets from the clipped origin. Valid source buffers retain
their existing success semantics while off-screen image area is not rasterized.

## Graphics stroke geometry update (2026-10-19)

`Renderer::stroke` computes far edges with checked conversion and saturation,
then delegates each edge to the clipped fill path. This preserves normal
outline behavior while keeping extreme rectangle dimensions bounded.

## Graphics gradient clipping update (2026-10-19)

`Renderer::vertical_gradient` and `Renderer::horizontal_gradient` iterate
only over the Canvas intersection, but derive interpolation from the full
logical rectangle so off-screen origins do not change visual output. The
shared Canvas clip helper uses i128 bounds to reject extreme dimensions
without overflow.

## Graphics transformed-triangle clipping update (2026-10-19)

The Renderer transformed-rectangle triangle helper clips its iteration bounds
to the Canvas before rasterization, sharing the widened edge arithmetic and
safe pixel boundary used by software 3D.

## Graphics software-3D clipping update (2026-10-19)

`Renderer::triangle_3d` bounds raster iteration by both the Canvas and depth
buffer rectangles and uses i128 edge/interpolation intermediates. This keeps
caller-owned software 3D safe for extreme projected coordinates while leaving
the future hardware/GPU backend boundary unchanged.

## Graphics contextual GPOS format-2 target update (2026-10-19)

The chaining format-2 path keeps first-input class coverage separate from
nested target coverage, allowing bounded single-positioning records to target
later input sequence indices without weakening class or table validation.

## Graphics contextual GPOS format-1 pair extension update (2026-10-19)

The chaining format-1 nested lookup-2 pair path is regression-tested through a
checked type-9 extension wrapper, reusing the same target, partner, pair-set
and value-record bounds.

## Graphics contextual GPOS format-1 pair update (2026-10-19)

The chaining format-1 path now shares the checked nested lookup-2 format-1
pair-positioning implementation with chaining format 2. Target coverage,
partner glyph, pair-set and value-record bounds are validated before applying
the first-glyph adjustment.

## Graphics contextual GPOS format-1 target update (2026-10-19)

The bounded chaining format-1 path keeps first-input coverage matching separate
from nested target coverage, so records targeting sequence indices 0 through 4
apply their single-positioning value to the selected glyph.

## Graphics contextual GPOS class-matrix extension update (2026-10-19)

Nested lookup-2 format-2 class-matrix pair positioning is regression-tested
through a checked type-9 extension wrapper. The wrapper target and nested
subtable bounds feed the same validated class/matrix/value-record path.

## Graphics contextual GPOS class-matrix pair update (2026-10-19)

Nested lookup-2 format-2 class-matrix pair positioning now shares the bounded
class-rule path. The selected target and partner classes, matrix dimensions,
value-record size and matrix extents are validated before applying the first
value record; malformed or unsupported nested forms fail closed.

## Graphics contextual GPOS class-rule pair update (2026-10-19)

Nested lookup-2 format-1 pair positioning is now available from the bounded
GPOS chaining format-2 class-rule path. Pair-set, partner glyph, value-format
and record extents are checked before applying the first-glyph adjustment;
broader nested pair/class forms remain outside this slice.

## Graphics contextual GPOS chaining format-2 update (2026-10-19)

The GPOS lookup type-8 format-2 path validates class definitions, class-set
rules, bounded backtrack/input/lookahead classes and nested lookup records.
Matched rules share the checked nested lookup-1 single-positioning path,
including type-9 wrappers; nested pair/class positioning remains outside this
slice.

## Graphics contextual GPOS chaining format-1 update (2026-10-19)

The GPOS lookup type-8 format-1 path validates bounded glyph-rule chains,
backtrack/lookahead context and nested lookup-1 single positioning. Direct and
type-9-wrapped nested lookups share the checked value-record path; nested pair
and class positioning remain outside this slice.

## Graphics contextual GPOS chaining update (2026-10-19)

GPOS lookup type 8 chaining-context format 3 shares the bounded contextual
dispatcher with type 7, including checked input/context coverage, nested
single/pair positioning, class matrices and extension wrappers.

## Graphics contextual GPOS class-pair update (2026-10-19)

Nested contextual type-2 pair positioning supports format-2 class definitions
and class-matrix value records with checked coverage, class and table bounds.
The bounded target/partner selection is shared with the contextual input
sequence path; broader nested lookup application remains outside this slice.

## Graphics contextual GPOS extension update (2026-10-19)

Contextual GPOS dispatch accepts a checked type-9 extension wrapper around a
nested type-7 subtable and then applies the existing bounded contextual paths.
Extension offsets and nested table bounds are validated before dispatch.

## Graphics two-coverage contextual GPOS guard update (2026-10-19)

The bounded contextual format-3 matcher validates up to two backtrack and two
lookahead coverage records using independently supplied neighboring glyphs.
The implementation remains explicitly bounded and fails closed when required
neighbors are unavailable; longer context is not implied.

## Graphics multi-input contextual GPOS pair update (2026-10-19)

Nested type-2 pair positioning can target each supported contextual input
index, using the following input glyph when available or the next shaped glyph
outside the matched sequence. The target adjustment and later-origin advance
propagation remain bounded and checked; broader nested lookup application is
not implied.

## Graphics contextual GPOS sequence-index-1 pair update (2026-10-19)

The bounded two-input contextual dispatcher can apply a nested type-2 pair
lookup to sequence index one, using the following shaped glyph as its checked
pair partner and propagating the resulting adjustment from that target.
Nested pair targets beyond index one remain outside this path.

## Graphics five-input contextual GPOS update (2026-10-19)

The bounded format-3 dispatcher supports five input coverages and can apply
nested type-1 positioning to sequence index four, propagating advance deltas
through later shaped glyph origins. Pair positioning remains restricted to the
two-input, sequence-index-zero path; longer context and broader nested lookup
application remain outside this implementation.

## Graphics four-input contextual GPOS update (2026-10-19)

The bounded format-3 dispatcher supports four input coverages and can apply
nested type-1 positioning to sequence index three, propagating advance deltas
through later shaped glyph origins. Pair positioning remains restricted to the
two-input, sequence-index-zero path; longer context and broader nested lookup
application remain outside this implementation.

## Graphics three-input contextual GPOS update (2026-10-19)

The bounded format-3 dispatcher supports three input coverages and can apply
nested type-1 positioning to sequence index two, propagating advance deltas
through later shaped glyph origins. Pair positioning remains restricted to the
two-input, sequence-index-zero path; longer context and broader nested lookup
application remain outside this implementation.

## Graphics single-input contextual GPOS update (2026-10-19)

The contextual format-3 dispatcher supports one-input coverage records through
the shaped-glyph pass, applying nested type-1 positioning and propagating
advance deltas. Adjacent context remains bounded to one checked backtrack and
one lookahead glyph; broader contextual lookup semantics remain outside this
path.

## Graphics contextual GPOS sequence-index-1 update (2026-10-19)

The bounded contextual format-3 dispatcher can apply nested type-1 single
positioning to sequence index one and propagate its advance through following
shaped glyph origins. Nested pair positioning remains restricted to sequence
index zero; later indices and broader contextual lookup application remain
outside this path.

## Graphics contextual GPOS composition update (2026-10-19)

Contextual format-3 dispatch accumulates multiple nested single or pair value
records targeting sequence index zero with saturating placement/advance
arithmetic. Other sequence indices and broader contextual lookup application
remain outside this bounded path.

## Graphics contextual GPOS guard update (2026-10-19)

Format-3 contextual positioning can validate one previous and one following
coverage against adjacent shaped glyphs, while retaining the context-free
two-input path. The bounded dispatch remains limited to sequence index zero;
longer context and multi-record lookup application are not implied.

## Graphics nested contextual GPOS update (2026-10-19)

Contextual format-3 records may now dispatch a bounded nested type-2
format-1 pair lookup for the first input sequence position, alongside nested
single adjustment. Pair-set coverage and value-record bounds are checked;
general contextual backtrack/lookahead and multi-record dispatch remain
outside this slice.

## Graphics contextual GPOS update (2026-10-19)

The bounded contextual positioning path recognizes format-3 records with two
input coverages and no backtrack/lookahead, then applies a nested type-1
format-1 adjustment to the first input. Nested extension wrappers are
validated through the shared lookup resolver; broader nested pair/context
composition is intentionally not implied.

## Graphics GPOS mark-to-ligature update (2026-10-19)

Lookup type 5 format 1 decodes mark arrays and bounded ligature component
records, resolving the selected class anchor against the mark anchor. The
current shaped-glyph representation does not encode a following mark's target
component, so the implementation selects the final ligature component; a
future cluster/component-carrying ABI can refine that choice.

## Graphics GPOS attachment extension update (2026-10-19)

Attachment lookup paths accept type-9 extension wrappers only when the
extension format and nested lookup type match the requested mark-to-base,
mark-to-mark or cursive path. The nested subtable offset and all enclosing
bounds are checked before decoding.

## Graphics GPOS script/LangSys attachment update (2026-10-19)

Mark-to-base, mark-to-mark and cursive positioning share a bounded feature
selector that resolves the requested script and LangSys records, including
required features. An absent script or language produces no attachment;
default shaping enumerates the feature list as before.

## Graphics GPOS lookup composition update (2026-10-19)

Selected `kern` lookups are evaluated in declared order. Matching single and
pair value records accumulate with saturating placement/advance arithmetic;
single and pair queries remain separated by the internal sentinel, and every
lookup/subtable path retains bounded validation.

## Graphics GPOS outline-resolved anchor update (2026-10-19)

Format-2 anchors resolve their contour-point index through `outline_points_glyph`
before attachment deltas are applied. Point count, glyph outline and table
bounds are checked, and missing or malformed outlines fail closed.

## Graphics GPOS attachment anchor update (2026-10-19)

Format-3 anchor decoding, including optional ppem device deltas, is shared by
mark-to-base, mark-to-mark and cursive attachment. Format-2 contour-point
anchors use the bounded outline decoder.

## Graphics GPOS format-3 anchor update (2026-10-19)

Mark-to-base anchor decoding supports format 3 with optional ppem device
deltas. Format 1 remains coordinate-only; format 2 contour-point anchors use
the bounded glyph outline points.

## Graphics GPOS mark-to-mark update (2026-10-19)

GPOS type-6 format-1 `mkmk` records now connect a second mark to a first mark
using validated class-matched anchors. The shaping pass applies the first
anchor minus second anchor delta to the second mark origin while retaining its
advance; alternate anchor formats remain outside this slice.

## Graphics GPOS cursive update (2026-10-19)

GPOS type-3 format-1 cursive records now connect adjacent covered glyphs using
validated entry/exit anchors. The right glyph receives the exit-minus-entry
origin delta and retains its advance; other attachment and anchor formats are
outside this slice.

## Graphics GPOS mark-to-base update (2026-10-19)

The shaping pass recognizes GPOS type-4 format-1 mark-to-base records in
`mark` and `mkmk` features. It validates coverage, class and anchor arrays,
then applies base-minus-mark anchor deltas to the mark origin while retaining
its advance. Other attachment and anchor formats remain outside this slice.

## Graphics GPOS single-adjustment update (2026-10-19)

GPOS type-1 format-1/2 subtables now produce bounded placement and advance
adjustments through the existing value-record/device-table decoder. The
shaping pass applies the result per glyph and shifts following origins by
advance deltas; attachment and other positioning lookup types remain outside
this slice.

## Graphics GPOS extension update (2026-10-19)

The GPOS shaping path unwraps type 9 extension records for nested type-2 pair
positioning subtables. The existing format-1/format-2 value-record parser and
device-table ppem handling are reused after validating the extension offset;
other positioning lookup types remain fail-closed.

## Graphics GSUB extension class update (2026-10-19)

Type 7 extension records for lookup types 5 and 6 can now route format-2
class-based contextual/chaining subtables through the existing checked class
matchers. The extension path preserves bounded class definitions, rules and
nested lookup references; malformed data remains fail-closed.

## Graphics GSUB extension contextual update (2026-10-19)

Type 7 extension records for lookup types 5 and 6 can unwrap bounded format-3
contextual/chaining subtables and route them through the existing coverage
matcher. Nested lookup references remain bounded by the active lookup list;
class-based extension forms remain fail-closed until separately implemented.

## Graphics GSUB extension ligature update (2026-10-19)

Type 7 extension records can now unwrap bounded type-4 ligature subtables in
addition to type-1 single substitutions. Matching returns the consumed
component count to the shaping pipeline; malformed or unsupported extension
forms remain fail-closed.

## Graphics GSUB extension update (2026-10-19)

The bounded GSUB shaping path unwraps type 7 extension records for nested
single substitutions, validating the extension offset and the nested format 1
or 2 coverage/substitute data before applying it. Other extension lookup types
remain fail-closed until separately implemented.

## Graphics GSUB composition update (2026-10-19)

The shaping path runs selected GSUB results through an eight-pass bounded
local pipeline. A single substitution can therefore feed the next selected
lookup, while ligature consumption ends the local sequence and repeated
no-op/cycle behavior cannot run indefinitely. Complete OpenType lookup
ordering, broader nested composition and hardware GPU acceleration remain
outside this slice.

## Graphics class-based chaining GSUB update (2026-10-19)

GSUB lookup type 6 format 2 now validates separate backtrack, input and
lookahead class definitions and applies matching chain rules with a nested
type-1 substitution at sequence index zero. Class-set, rule, sequence, lookup
and glyph ranges are bounded. Broader nested lookup composition, complete
feature ordering and hardware GPU acceleration remain outside this slice.

## Graphics chaining GSUB update (2026-10-19)

GSUB lookup type 6 format 3 now shares the bounded backtrack/input/lookahead
coverage matcher with contextual shaping. A matching chaining record can
invoke a nested type-1 substitution at sequence index zero; all table and
glyph ranges are validated. Class-based chaining, broader nested lookup
composition, complete feature ordering and hardware GPU acceleration remain
outside this slice.

## Graphics class-based contextual GSUB update (2026-10-19)

GSUB lookup type 5 format 2 now supports checked class-definition lookup,
class-set rule selection, subsequent input-class matching and a nested type-1
substitution at sequence index zero. Rule, class-set, nested-lookup and glyph
ranges are bounded. Chaining substitution, complete feature ordering and
hardware GPU acceleration remain outside this slice.

## Graphics contextual GSUB update (2026-10-19)

The bounded GSUB path supports contextual lookup type 5 format 3 coverage
matching across backtrack, input and lookahead glyphs. A matching record may
invoke a nested type-1 single substitution at sequence index zero; all
coverage, sequence, lookup and glyph ranges are validated. Class-based/contextual
formats 1/2, chaining substitution, complete feature ordering and hardware GPU
acceleration remain outside this slice.

## Graphics GSUB single-substitution update (2026-10-19)

The bounded GSUB path accepts lookup type 1 formats 1 and 2 for selected
Script/LangSys features (`ccmp`, `locl`, `smcp`, `subs`, `sups`, `case`, and
`cpsp`). Coverage indexes, format-2 substitute arrays and resulting glyph IDs
are range-checked before a shaped glyph is emitted. Contextual/chaining GSUB,
complete feature ordering and hardware GPU acceleration remain outside this
slice.

## Graphics named LangSys update (2026-10-19)

`TextShapingOptions` carries optional ScriptList and named LangSys tags plus
the ppem needed by device tables. The shaping path resolves the requested
language record with checked bounds for both GSUB `liga`/`rlig` and GPOS
`kern`; a missing language record does not silently use the default or another
language. Contextual/complex GSUB/GPOS, broader script data and hardware GPU
acceleration remain outside this slice.

## Graphics script-scoped GPOS update (2026-10-19)

The script-aware shaping path now passes the selected ScriptList/default
LangSys through GPOS `kern` pair lookup selection as well as GSUB. The
`shape_text_ligatures_with_script_and_ppem` entry point makes both the script
tag and ppem required when device deltas are needed. Missing scripts do not
fall back to another script. Named language systems, contextual/complex
GSUB/GPOS and hardware GPU acceleration remain outside this slice.

## Graphics GSUB script-selection update (2026-10-19)

`TrueTypeFace::shape_text_ligatures_with_script` selects a requested ScriptList
record and its default LangSys before resolving bounded `liga`/`rlig` feature
records and type-1 lookups. It does not fall back to another script when the
requested tag is absent. The legacy shaping API retains its existing feature
list behavior. Language-specific LangSys selection, GPOS script scoping,
contextual/complex GSUB and hardware GPU acceleration remain outside this
slice.

## Graphics GPOS device-table update (2026-10-18)

`TrueTypeFace::shape_text_ligatures_with_ppem` is the explicit ppem-aware
entry point for GPOS device records. Formats 1, 2 and 3 are decoded with
checked ranges and signed packed deltas; each delta augments its matching
x/y-placement or x/y-advance value before the existing font-unit scaling and
origin propagation. The no-ppem shaping path returns a fail-closed error when
a device record is encountered. Variation-index device semantics, full script
shaping and hardware GPU acceleration remain outside this slice.

## Graphics GPOS y-value update (2026-10-18)

GPOS pair records now carry bounded y-placement and y-advance values through
the shaped glyph origin path, with font-unit-to-pixel scaling and propagation
to following origins. Device-table deltas and full vertical layout semantics
are not implied; the QEMU/VirGL hardware presentation boundary is unchanged.

## Graphics GPOS class positioning update (2026-10-18)

GPOS pair positioning now supports bounded format-2 class matrices in addition
to format 1 pair sets. Class-definition formats 1/2 and matrix dimensions are
validated before applying scaled x adjustments; unsupported value/device
semantics remain fail-closed. Full script shaping and the QEMU/VirGL hardware
presentation boundary are unchanged.

## Graphics GPOS positioning update (2026-10-18)

The shaped text path optionally follows the GPOS `kern` feature to type-2
pair-positioning format 1 and applies bounded x-placement/x-advance records in
scaled pixels. Unsupported lookup/value formats remain fail-closed; full GPOS
device tables and script shaping are not implied. The QEMU/VirGL hardware
presentation boundary is unchanged.

## Graphics variation-aware shaping update (2026-10-18)

Shaped glyph IDs can now carry normalized gvar coordinates through metric
calculation, outline construction and antialiased rasterization. The base path
does not apply variation deltas; the variation path applies them once and
validates coordinate/table bounds. Full script shaping and the QEMU/VirGL
hardware presentation boundary remain separate.

## Graphics shaped-glyph raster update (2026-10-18)

The font boundary now supports direct glyph-ID outline decoding and
antialiased rasterization, with `render_shaped_text` carrying GSUB-selected
IDs from shaping into the renderer. Unicode-character wrappers remain intact;
variation-aware shaped rasterization and full script shaping are not implied.
The QEMU/VirGL hardware presentation boundary is unchanged.

## Graphics GSUB shaping update (2026-10-18)

The borrowed TrueType/OpenType boundary now exposes `ShapedGlyph` and a
bounded GSUB ligature path. It follows `liga`/`rlig` feature records to type-1
ligature lookups, validates coverage and component sequences, and derives the
selected glyph's horizontal metric. It does not yet rasterize arbitrary shaped
glyph IDs or implement full script shaping; the QEMU/VirGL hardware boundary
is unchanged.

## Graphics directional text update (2026-10-18)

`TrueTypeFace::layout_text_bidi` resolves bounded strong RTL/LTR runs and
neutral characters before emitting visual placements. `TextDirection::classify`
recognizes common RTL blocks and Unicode alphanumerics; punctuation and
combining marks remain neutral. Full UAX#9, script shaping, and OpenType
feature application are not implied, and the QEMU/VirGL hardware boundary is
unchanged.

## Graphics hint control-flow update (2026-10-18)

The bounded TrueType interpreter now recognizes `IF`/`ELSE`/`EIF` and
relative `JMPR`/`JROT`/`JROF`. Branch scanning is instruction-span-aware so
immediate PUSH data cannot be mistaken for opcodes; nesting and jump targets
are bounded by the copied program, and malformed control flow fails closed.
Function calls, full projection math and hint interpolation are not implied.

## Graphics storage/CVT hint update (2026-10-18)

The bounded hint interpreter exposes caller-owned CVT and storage resources:
`MIAP`/`MDAP` validate point and CVT indices, while `WS`/`RS` validate storage
indices. Missing resources and unsupported instructions fail closed; the
software renderer and QEMU/VirGL presentation boundary are unchanged.

## Graphics CVT hint update (2026-10-18)

The bounded hint interpreter now supports `MDAP` point selection and `MIAP`
placement from caller-owned CVT values along the active freedom axis. Missing
CVT data and unsupported hint resources fail closed; software rendering and
the QEMU/VirGL presentation boundary are unchanged.

## Graphics hint-state execution update (2026-10-18)

The font boundary now executes a bounded coordinate-independent hint subset
covering vector selection, rounding state, stack primitives and PUSHB/PUSHW.
Point movement, storage, CVT and control flow fail closed; the software
renderer and QEMU/VirGL presentation boundary are unchanged.

## Graphics hinting boundary (2026-10-18)

`TrueTypeFace::glyph_hint_program` provides a bounded, validated copy of
simple-glyph instruction bytes; execution is deliberately absent, and
composite programs fail closed. The software raster path and QEMU/VirGL
presentation boundary remain unchanged.

## Graphics phantom metrics update (2026-10-18)

`TrueTypeFace::advance_width_with_variation` retains simple-glyph phantom
points while applying `gvar` deltas, deriving varied advance width from the
right and left phantom positions. Incomplete legacy LSB storage preserves the
base advance path and fails closed for variation metrics; QEMU/VirGL output is
unchanged.

## Graphics composite variation update (2026-10-18)

Composite `gvar` processing now treats packed variation point numbers as
component indices, recursively applies nested component variation, and shifts
each bounded component output range by its checked X/Y delta. Phantom metrics
and hinting remain outside this renderer boundary; QEMU/VirGL presentation is
unchanged.

## Graphics shared-point variation update (2026-10-18)

Simple-glyph `gvar` tuples may now use a checked glyph-level packed shared
point set, including its all-points form. Composite variation remains
unsupported and fails closed; the QEMU/VirGL hardware presentation boundary
is unchanged.

## Graphics variation update (2026-10-18)

The simple-glyph `gvar` path now supports embedded-peak intermediate regions
using checked start/peak/end ranges and piecewise normalized F2DOT14 scalar
blending. Shared-point sets and composite variation still fail closed; the
QEMU/VirGL hardware presentation boundary is unchanged.

## Graphics font variation boundary (2026-10-18)

`libwoven::graphics::TrueTypeFace::outline_points_with_variation` applies
embedded-peak `gvar` tuples to simple glyph points using normalized F2DOT14
coordinates. Composite glyph variation, intermediate regions and shared-point
sets fail closed until separately implemented; the compositor and QEMU/VirGL
hardware boundary are unchanged.

## Graphics backend boundary (2026-10-18)

The kernel VirtIO-GPU path keeps the existing software/2D scanout as the
accepted fallback. Its Stage 20.14 VIRGL probe negotiates optional resource
blobs, creates bounded context/resource command streams, and can encode a
surface/framebuffer/RGBA-clear sequence. QEMU's default GL device accepts the
transport path but does not accept the tested 3D resource through classic
`SET_SCANOUT`; its opt-in guest blob path is not accepted by the VirGL context.
Blob-backed presentation therefore remains capability-gated and unclaimed
until a visual QEMU frame is verified.

## Live boot log startup candidate (2026-10-16; physical retry correction 2026-10-17)

Production boot queues Stage 13.19 USB live-log setup immediately before
attempting to start the Stage 20.15 WovenShell session. File creation and the
initial FAT flush run in a background I/O service, because they may take time
on physical USB media. The worker is queued even before USB enumeration is
visible and retries discovery/setup failures. Physical USB enumeration now
also retries every 300 timer ticks until a storage device is actually
published; this covers devices that settle after the first probe and avoids
the existing-HID fast path suppressing later mass-storage discovery. The
acceptance self-test retains the synchronous start path. Focused Clippy and
Stage 13.18/13.19 QEMU gates pass, but physical USB boot still returned no
`LOGS/BOOT0001.TXT` on two sticks; serial capture remains required. See the
[startup audit](audit-live-boot-log-startup-2026-10-16.md).

Driver bring-up and xHCI initialization now emit phase checkpoints before
potentially long waits. Serial transmit polling is bounded in all builds, with
log bytes retained in the in-memory ring even if no UART responds. This does
not make a USB log possible while xHCI is hung; external serial capture is
needed for a permanently stalled controller.

## Stages 1â€“10 release evidence integrity (2026-10-16)

Memory, storage, live-network and Stage 6 hotplug QEMU gates share
`scripts/qemu_validation.py`: exact successful exit, line-anchored required
markers, and rejection of tagged failure/timeout or kernel panic anywhere in
serial output. Expected user faults remain valid isolation evidence. Negative
host tests exercise every consumer's imported validator. This changes no kernel
ABI or lifecycle; full preservation is pending, not release acceptance. See
the foundation release-validation audit and stages 1â€“10 release ledger.

## Stage 19 WovenCalculator (2026-10-14)

`apps/woven-calculator` is a separate Ring-3 crate with a bounded checked
arithmetic core, a console mode, and an optional GUI mode. It uses the same
static ET_EXEC linker contract and WAPP signing path as the existing external
application. Its 1/2/4-CPU signed QEMU evidence is recorded in the Stage 19
calculator audit. This is the first system application increment; the rest of
the roadmap application suite and catalog/boot-image installation remain open.

`apps/woven-terminal` is the second application increment. It provides bounded
console commands over `libwoven` file descriptors, delegates arithmetic to the
calculator core, and passes signed 1/2/4-CPU Ring-3 execution. Its listing and
file-output caps are documented in the Stage 19 terminal audit; interactive
stdin and boot-catalog installation remain open.

`apps/woven-files` is the third application increment: a read-only bounded
directory lister and file previewer using the same verified external-image
path. It passes signed 1/2/4-CPU execution; GUI, boot catalog, and mutation
workflows remain open. See the Stage 19 WovenFiles audit.

`apps/woven-monitor` is the fourth increment. It reports bounded UTC clock,
authorized compositor geometry, and `/mnt` entry-count diagnostics without
write or privilege authority. Its signed 1/2/4-CPU execution evidence is
recorded in the Stage 19 WovenMonitor audit.

`apps/woven-settings` is the fifth increment. It reports bounded process
identity, UTC clock, and compositor display policy without mutating settings or
requesting privileged authority. Its signed 1/2/4-CPU execution evidence is
recorded in the Stage 19 WovenSettings audit.

`apps/woven-security` is the sixth increment. It reports bounded process
identity and security-ledger entries when the existing inspect authority is
available, without granting or revoking policy. Its signed 1/2/4-CPU execution
evidence is recorded in the Stage 19 WovenSecurity audit.

`apps/woven-editor` is the seventh increment. It provides bounded text preview
and capability-enforced file writing through the safe `libwoven` file-write
wrapper. Its signed 1/2/4-CPU execution evidence is recorded in the Stage 19
WovenEditor audit.

`apps/woven-images` is the eighth increment. It provides bounded image
inventory and common-header inspection without decoding untrusted pixel data.
Its signed 1/2/4-CPU execution evidence is recorded in the Stage 19 WovenImages
audit.

`apps/woven-update` is the ninth increment. It provides bounded read-only
update preflight and leaves installation, activation, and rollback to a future
privileged update service. Its signed 1/2/4-CPU execution evidence is recorded
in the Stage 19 WovenUpdate audit.

`apps/woven-store` is the tenth increment. It provides bounded read-only
installed-catalog browsing and leaves package acquisition and mutation to a
future privileged service. Its signed 1/2/4-CPU execution evidence is recorded
in the Stage 19 WovenStore audit.

`scripts/package-system-apps.py` is the release packaging boundary for these
ten accepted apps. It builds each app in its own workspace, signs the ELF
with WAPP v1, checks the envelope before publication, and emits hashed package
and `/share/applications` catalog artifacts in one install-plan manifest. The
manifest is consumed by the feature-gated multi-package RAM installer; the
remaining roadmap applications and full GUI integration remain open.

## Stage 21.1 WovenPackages bounded envelope

`libwoven/src/package.rs` defines the first WovenPackages boundary as a
version-1 `WPKG` envelope. Its fixed header carries bounded manifest,
application, and resource lengths followed by a 64-byte signature field. The
manifest is length-prefixed and bounded to 4096 bytes; names are limited to 48
safe ASCII bytes, dependencies to 16 entries, applications to 64 MiB, and
resources to 16 MiB. Parsing checks magic/version, arithmetic overflow, exact
total length, manifest structure, names, dependency count, and nonzero package
version before exposing borrowed payload slices. The parser preserves the
signature bytes and immutable signed-prefix view, but deliberately does not
claim cryptographic authentication: the package service must verify the
signature over its defined envelope domain before installation. No package
producer, persistent installer, update/removal transaction, sandbox launcher,
or trust-root policy is implied by this parser increment.

`tools/wpkg-packager` is the producer boundary for WPKG v1. It preserves the
existing WAPP producer and accepts an ELF, package name/version, capability
bitmap, bounded dependency list, and optional resource files. The signature is
Ed25519 over `SHA-256("WOVENHAT-WPKG-V1" || immutable_header_prefix ||
manifest || application || resources)`, using the same provisioned application
trust-root model as signed applications. The kernel Stage 21.2 verifier checks
this digest before a future package service may install anything; producer
output alone does not grant installation authority.

`libwoven/src/store.rs` is the bounded WovenStore transaction core. A store has
32 catalog slots and an explicit allowed-capability mask. Each transaction
works on a copied catalog and can install a newer version, reject stale or
duplicate versions, require already-installed dependencies, reject undeclared
capabilities, and refuse removal while another entry depends on the target.
Only `commit` publishes the working snapshot; `rollback` returns the original
snapshot. This is deliberately independent of storage I/O: the future package
service must authenticate WPKG first, then persist the committed catalog and
payload files through capability-gated WovenFS transactions.

`Transaction::install_batch` resolves up to the catalog capacity of requests
with bounded repeated passes. A request blocked only by a missing dependency
is deferred; any other error aborts immediately, and a pass with no progress
returns `MissingDependency` for cycles or absent dependencies. The transaction
does not publish until the caller commits, so a failed batch cannot expose a
partial catalog.

The catalog record codec uses `WSTC` version 2 and a fixed per-entry layout;
the record carries the allowed capability mask, entry count, exact byte length,
all bounded dependency metadata, and an FNV-1a-style 32-bit checksum over the
header and entries. It is intentionally a codec, not a storage write: the
package service must commit it through the authorized WovenFS writer and
validate the decoded record before publishing the catalog.

Stage 21.6â€™s kernel probe writes `/wfs/packages/catalog.wstc` only after
`UpdateState::authorized_writer` confirms the current task has
`Capability::StorageIo`. It uses native WovenFS journaled path writes and
direct checksum-verified readback, so the tested boundary does not rely on a
stale VFS cache. The current probe persists only catalog metadata; application
payload/resource publication and a long-lived WovenPackage service still need
their own transaction and recovery design.

Stage 21.7 adds the first payload publication path. `package_service::install`
receives a package only after `package::verify`, derives the application
digest, stages bounded chunks below `/wfs/packages/apps` and
`/wfs/packages/resources`, then writes the WSTC catalog. Chunk names include
package name and version, so an update can stage a new generation without
destroying the old payload first. Any chunk or catalog error removes chunks
created by the attempt. The current service is a kernel-side acceptance
boundary; long-lived userspace ownership and IPC are still separate work.

Stage 21.8 adds `/wfs/packages/install.wpkj`, a fixed-size checksummed intent
record. The service writes `Prepared` before payload chunks, writes the catalog,
then marks the intent `Committed` and removes the journal. On startup or before
another install, recovery removes prepared chunks unless a committed intent is
already represented by the catalog at the same package version; it then clears
the intent. This closes the tested orphan-cleanup window without claiming that
multi-package or power-loss recovery across every catalog mutation is complete.

Stage 21.9 extends WSTC entries with application/resource chunk counts and
extends WPKJ with the superseded generation. Stage 21.10 adds one bounded
rollback generation: the previous catalog is durable before a new transaction,
the previous payload remains available, and a checksummed rollback intent makes
restoration idempotent. The next transaction garbage-collects only generations
that are neither active nor the retained rollback generation.

Stage 21.11 turns package capability declarations into launch authority.
`wovenguard::package_sandbox` rejects capabilities outside the user-domain
ceiling, narrows file access to system/temporary/mounted/home scopes, maps
declared device classes, and permits only application-class service discovery.
`task::spawn_user_process_with_package_capabilities` applies that immutable
profile before publishing the new address space and destroys invalid images.
This is the kernel launch boundary; a long-lived userspace WovenPackage IPC
broker still needs to be built above it.

## Stage 22.1 userspace service IPC ABI (2026-10-20)

The service registry's first userspace boundary is now explicit. Syscall 211
creates an endpoint owned by the calling process. Syscalls 212â€“214 publish,
unpublish, and discover a bounded service name; all four require
`Capability::Ipc`. The publish argument carries the endpoint handle and
client-rights mask, while the kernel validates the raw handle, copies the
service name from userspace before taking the IPC registry lock, and returns
only process-local handles. `libwoven` provides typed wrappers that preserve
the kernel's error sentinel contract.

The accepted QEMU probe exercises the underlying registry's owner cleanup,
rights filtering, namespace isolation, and endpoint lifetime behavior on 1, 2,
and 4 CPUs. The ABI is groundwork for moving WovenPackage ownership out of the
kernel; a long-lived Ring-3 broker/client, service-specific message protocol,
and physical hardware qualification remain open.

Stage 22.2 adds the usable message path. Syscall 215 copies a bounded payload
from userspace and calls the existing handle-addressed IPC send path; syscall
216 receives one queued payload after validating the caller's endpoint rights,
capacity, and output range. The receive output begins with the sender process
ID, allowing a broker to route a correlated response through the existing
process mailbox. The ABI is deliberately nonblocking and returns the payload
length, matching the established queue contract; a later service layer can
build event-driven retry/wait behavior on the existing blocking kernel
primitives. `libwoven` exposes both operations. The next
boundary is a versioned WovenPackage service protocol and a real Ring-3 broker.

`libwoven::package_ipc` is the first protocol layer. Its `WPKI` v1 request
frame carries an operation, request identifier, and validated package path;
responses carry the identifier, bounded status, and value. Install, remove,
rollback, and query are explicit operations. Package bytes do not travel in
the fixed IPC message, avoiding a second unverified payload path. The codec is
allocation-free and covered by host tests; the next acceptance must exercise
the codec from a real Ring-3 broker and connect each operation to the existing
authenticated WovenPackage transactions.

## Stage 22.3 Ring-3 WovenPackage broker boundary (2026-10-20)

`apps/woven-package` is a signed external ELF service, not kernel code. It
creates and publishes `woven.package` with `Capability::Ipc`, receives
`WPKI` requests through the discovered endpoint, and routes responses using
the sender process ID prefixed by the handle-receive ABI. Request and response
buffers live in static bounded storage because WovenHat user stacks are small.
The service exits only through normal teardown in the fixture; the kernel reaps
it and unregisters its service owner before acceptance completes.

The current dispatch implements a bounded storage-backed existence query through
the userspace directory authority. In `system-apps` boots, the signed service
image is injected by `WOVENHAT_PACKAGE_IMAGE`, spawned after catalog install,
and publication is required before boot proceeds.
Stage 22.5 completes that transaction boundary. Syscall 217 accepts an
operation and a bounded source path, checks `FileRead`, `FileWrite`, and
`StorageIo` on the current broker task, and then creates the private
`AuthorizedWfsWriter`. The Ring-3 broker uses it for install, remove, and
rollback; package bytes never cross the IPC frame. Install reads at most one
256 KiB VFS node, verifies the WPKG envelope, and calls the existing
authenticated WovenFS journal/catalog/chunk publication path. Remove and
rollback use recovered catalog state and durable rollback/install intent
records before committing the next catalog generation. The production
`system-apps` service receives all three capabilities, while the query-only
Stage 22.3 fixture remains least-privilege.

The accepted transaction gate formats a disposable ATA volume and exercises
install, durable catalog verification, removal, rollback, and restoration on
1/2/4 CPUs. The source-path limit is the normal 512-byte syscall bound; the
single-package ingress limit is the 256 KiB VFS node capacity; native package
publication remains chunked by the WovenFS file limit. This is a transaction
authority, not yet a complete update daemon: key rotation, multi-package
power-loss timing, larger/chunked source ingress, and physical USB validation
remain explicit prerequisites for a release claim.

The service-enabled kernel task stack is 256 KiB. This is an explicit resource
budget for the bounded WovenStore/WovenFS transaction call chain and is
validated by the stack canary on the focused 1/2/4-CPU acceptance gate.
The 256 MiB Stage 24.59 regression profile selects the same 256 KiB bound
explicitly, rather than inheriting the larger production `system-apps` stack
profile, so its bounded low-memory fixture retains boot-mapping headroom.

## Stage 23.1 controlled WovenAI broker boundary (2026-10-20)

`libwoven::ai_ipc` defines the `WAI1` control protocol with fixed headers,
UTF-8 input validation, a 224-byte input ceiling, and fixed-size responses.
`apps/woven-ai` is a Ring-3 service that publishes `woven.ai`. Its `Describe`
operation returns the broker ABI version; `Complete` returns `NoProvider` until
Stage 23.2/23.3 supplies a local or external provider. This keeps the first
AI increment honest: a broker exists, but no model execution is claimed.

Syscall 218 is the controlled client entry point. It requires both
`Capability::Ipc` and `Capability::Ai` before copying and sending the bounded
request to a discovered endpoint. The acceptance probe runs as a normal user
process whose declared package capabilities include exactly `Ipc` and `Ai`;
the broker runs as a least-privilege service with `Ipc` only. The WovenGuard
user ceiling permits `Ai`, while the SystemService ceiling deliberately does
not, preventing accidental AI authority from being inherited by unrelated
services.

The accepted 1/2/4-CPU gate proves service publication, capability-gated
request delivery, ABI response correlation, explicit no-provider behavior,
and teardown. It does not provide model inference, file/network/secret/device
access, assistant actions, or physical hardware qualification.

## Stage 23.2 bounded WovenAI local provider (2026-10-20)

`libwoven::ai_model` is the first provider implementation. It is intentionally
bounded and deterministic: fixed integer weights, a fixed feature vocabulary,
no allocation, ASCII-insensitive matching, and a packed class/confidence
result. Its `MODEL_ABI` is advertised in the local broker's `Describe` value
alongside `LOCAL_PROVIDER_FLAG`; the normal broker descriptor remains the
Stage 23.1 broker ABI.

The provider runs in Ring 3 inside `apps/woven-ai` and receives only the WAI1
request bytes already validated by the protocol. It has no model-file loader,
filesystem, network, secret, device, or hardware-accelerator path. This is a
release-safe local inference baseline, not a claim of general neural-model
support. Larger model formats and accelerator backends require later bounded
provider stages and their own acceptance gates.

## Stage 23.3 WovenAI provider-plugin boundary (2026-10-20)

`libwoven::ai_provider` defines a provider descriptor and result flags over
the existing bounded WAI1 frames. `apps/woven-ai --cloud-plugin` is a separate
Ring-3 provider process; `apps/woven-ai --cloud` is a broker mode that forwards
requests to the discovered provider. The broker has only `Ipc`. The provider
is the only process holding `Secrets`, and reads its owner-bound WovenVault
handle locally; no credential bytes enter a WAI1 frame or broker memory.

The accepted fixture also declares `NetworkIo` so a future provider can attach
its own bounded transport, but this stage intentionally does not pretend that
an offline deterministic fixture is an HTTPS/TLS cloud implementation. Real
endpoint configuration, TLS validation, credential provisioning, retries,
quotas, and billing/availability policy require a subsequent provider
hardening gate.

## Stage 23.4 WovenAI capability security (2026-10-20)

The AI security gate uses the same WovenGuard `package_sandbox` and device
authorization path as every other Ring-3 package. The local profile is
`Ipc + Ai` only; its file masks and device mask are empty. The cloud profile
explicitly adds `Secrets + NetworkIo`, which produces a network-only device
mask and still has no file, camera, microphone, USB, or generic-device access.
The SystemService domain ceiling excludes `Ai`, `Secrets`, camera, and
microphone authority. These checks ensure a model receives only declared
capabilities and cannot widen its authority through the broker.

## Stage 23.5 bounded semantic search core (2026-10-20)

`libwoven::semantic_search` is an allocation-free fixed-capacity index for
application, file, setting, message, document, and command entries. Titles
are UTF-8 validated and normalized into bounded ASCII terms. Search ranks by
the number of covered query terms, with stable insertion-order tie handling;
there is no unbounded heap, model embedding, or implicit filesystem scan.
Entries carry an owner and private entries are filtered unless the query owner
matches. The WSI1 fixed-format snapshot includes an FNV-1a checksum and
round-trips through bounded storage; upsert and removal never expose a partial
index. Source adapters, ranking evaluation, and a user-facing service remain
separate work.

## Stage 23.6 WovenAssistant permissioned action planner (2026-10-20)

`libwoven::ai_assistant` stores at most eight fixed-size actions, rejects
empty/oversized/non-UTF-8/NUL-containing targets, maps each action to an
explicit `Permission`, and requires confirmation for side effects. The planner
never calls an OS executor; the eventual assistant service must pass its result
through the existing permission broker and capability policy before invoking a
platform API. Cancellation, owner teardown, audit ordering, and generation-
safe permission handles remain executor requirements.

## Stage 23.7 advisory WovenAI security monitor (2026-10-20)

`libwoven::ai_monitor` consumes copied audit events and emits fixed-size
severity/explanation findings. It is intentionally unable to grant, revoke,
or enforce capabilities; it has no executor or device API. WovenGuard and the
existing audit ledger remain authoritative. Process faults and denied sandbox
events receive stronger advisory severity, while the monitor preserves the
original `allowed` bit rather than rewriting policy decisions.

The feature-gated `stage19-app-catalog-test` now exercises the install-plan
boundary for all ten accepted packages together. It verifies each WAPP,
rejects a mutation, installs and reads each package back through the VFS,
passes the bytes through the verified ELF loader, and publishes the matching
catalog descriptor only after those checks. The 1/2/4-CPU evidence is in the
Stage 19 system-app catalog audit. This is not ordinary-boot installation:
rollback, trusted catalog publication, and image/installer ownership remain
required for production deployment.

The `system-apps` image feature is the first production-image consumer. It
requires a compile-time provisioned application public key, validates every
WAPP and normal ELF load before VFS mutation, checks path collisions, and
rolls back entries created by a failed publication. Acceptance harnesses pass
the deterministic test key explicitly; production provisioning must replace
it. The current publication is RAM-backed at boot, so persistent update,
power-loss rollback, and key lifecycle policy remain open.

## Stage 24.60 release-key provisioning boundary

`tools/wapp-packager` has two explicit modes. Test/development invocations may
use the deterministic fixture seed; production invocations are marked with
`WOVENHAT_APP_RELEASE_MODE=1` and fail closed unless both
`WOVENHAT_APP_SIGNING_SEED_HEX` and `WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX` are
present. The packager derives the public key from the signing seed and rejects
any mismatch. `package-system-apps.py`, `build-package-service.py`, and the
release fallback propagate this mode. Seeds never enter WAPP metadata or
catalog files. Secret custody, rotation/revocation, and physical qualification
remain deployment responsibilities.

## Stage 27.3 signed update-package producer (2026-10-21)

`tools/wupd-packager` is the producer for the kernel's version-1 `WUPD`
envelope. It keeps the generation and image length bounded, stores the exact
SHA-256 image digest, and signs a domain-separated digest over the immutable
header and image with Ed25519. Release mode requires both the signing seed and
the matching provisioned trusted public key; development fallback is limited
to the deterministic fixture key. The tool does not install or activate an
image. Long-lived service ownership, slot storage, boot handoff, power-loss
recovery, and physical qualification remain separate prerequisites.

## Stage 27.4 bounded update IPC contract (2026-10-21)

`libwoven::update_ipc` defines the future service's fixed-capacity `WUDI`
request/response boundary. `Stage` carries a positive generation and a
candidate path confined to `/updates/`; `Activate`, `BootSucceeded`, and
`BootFailed` carry only slot 0 or 1; `Query` carries no mutable lifecycle
fields. The response retains the request identifier and returns bounded
status/value fields. No image bytes cross IPC. This contract is intentionally
separate from the still-unimplemented capability-gated syscall, service
ownership, durable image-slot write, and bootloader handoff.

## Stage 27.5 end-to-end WovenUpdate IPC acceptance (2026-10-22)

The Stage 27.5 acceptance image packages the signed `woven-update` ELF and
loads it through the normal verified userspace path. The kernel acceptance
client discovers `woven.update`, sends a valid WUDI Query through the real
service endpoint, checks the activated-slot response, sends a truncated frame,
checks the fixed bad-request response, and then tears down the service. The
1/2/4-CPU gates pass. The daemon receive buffer is 256 bytes because the
kernel IPC syscall caps one transport message at 256 bytes; WUDI's larger
codec bound remains useful for parser validation but cannot bypass that
transport limit.

## Stage 27.5 privileged update-service transaction boundary (2026-10-21)

`apps/woven-update/src/service.rs` provides the bounded userspace service
process slice. `--service` publishes `woven.update`, receives the fixed-size
WUDI request, converts only the validated candidate path to UTF-8, and calls
the kernel-owned `libwoven::update_transaction` boundary. It uses fixed
request/response buffers, returns deterministic malformed-request and
transaction-failure responses, yields when the nonblocking receive queue is
empty, and exits on an unreplyable client so a supervisor can restart it. This
keeps update policy and capability authority in the kernel while giving the
userspace daemon a stable IPC owner.

The service-process slice has host tests and freestanding warning-denying
Clippy evidence. `start_production_system_services` now starts and verifies
both `woven-package` and `woven-update` before boot continues. The shell QEMU
gate passed on 1/2/4 CPUs with both publication markers, and the dedicated
Stage 27.5 gate proves the client/server exchange. The production slot
backend, bootloader handoff, power-loss recovery, and physical qualification
remain separate gates.

`kernel/src/update_service.rs` is the first executable service boundary. It
requires `FileRead`, `FileWrite`, and `StorageIo`, validates `/updates/`
source paths, verifies the signed WUPD envelope, writes the inactive slot
before publishing the next WUAB record copy, and exposes activation,
boot-success, boot-failure, and query lifecycle operations through syscall 239
and the `libwoven::update_transaction` wrapper. Record loading accepts one
valid copy for recovery, chooses the highest sequence when both are valid, and
rejects equal or unusable copies.

The Stage 27.5 QEMU gate (`scripts/test-stage27-5-update-service.py`) passes
on 1/2/4 CPUs and retains serial/QEMU/disk evidence. The gate uses a signed
small image because current WovenFS `MAX_FILE_SIZE` is 4 KiB. The service
therefore rejects larger images explicitly; it does not claim a production
boot-slot backend. The long-lived WovenUpdate userspace daemon, full IPC loop,
multi-file/block-device image store, bootloader target/attempt handoff,
power-loss injection/recovery, and physical USB qualification remain open.

## Release validation (2026-10-14)

## Update/recovery track â€” bounded A/B activation state (2026-10-20)

`kernel/src/update.rs` is the first Stage 27 increment. It provides an
allocation-free two-slot transition object: staged candidates must advance the
generation and fit the bounded image/digest contract; activation records the
next boot without replacing the known-good slot; three failed attempts mark
the candidate bad and roll back; and only an explicit post-boot health result
commits the new active slot. Its bounded version-1 `WUPD` envelope verifies
the immutable manifest and image digest under a domain-separated Ed25519
signature before admission. Service ownership, durable WovenFS records,
power-loss recovery, installer integration, and physical qualification remain
required.
The state object also exposes a fixed-size `WUAB` record codec with a sequence
and CRC-32. It rejects malformed or inconsistent snapshots and is intended for
dual-copy service commits; it does not perform storage I/O itself.

The Stage 27.2 native adapter in `wfs.rs` supplies the next boundary: a
capability-issued update writer commits exact-length state files through the
serialized WovenFS mutation path, and a direct native reader verifies them
after remount. `update::select_record` chooses the highest valid sequence and
fails closed when both copies are invalid or tied. This proves durable record
handling on the accepted ATA/WovenFS scratch path, but not power-loss timing,
boot-slot installation, or a complete userspace update daemon.

The update service boundary is capability-gated: `update::authorized_writer`
checks the current task for `Capability::StorageIo`, and only then receives
the private `wfs::AuthorizedWfsWriter` token. Commit/read helpers accept that
token rather than a path-only authority. The current acceptance invokes this
from the kernel test service; production still needs a long-lived userspace
service lifecycle and an IPC/syscall protocol that delegates this authority.


The repository release chain passed on the current tree, including warning-
denying kernel/host Clippy, all host integration tests, 1/2/4-CPU debug
memory/storage/network gates, legacy PIC, 4-CPU release gates, release build,
and shell smoke. It now also runs app-specific warning-denying Clippy and the
signed five-application catalog gate on 1/2/4 CPUs. The release ZIP includes
the signed WAPP packages, catalog descriptors, and complete SHA-256 inventory.
The durable report and image manifest are recorded by the
[release validation audit](audit-release-validation-2026-10-14.md) and the
[system-app release audit](audit-release-system-apps-2026-10-14.md). This is
software/QEMU qualification only; native hardware and destructive media limits
remain explicit.

## Stage 14.5D network revalidation (2026-10-14)

The live WovenGuard transport/path suite passes on 1/2/4 CPUs, including
DHCP, DNS, ICMP, host-verified UDP/TCP round trips, and SMP/TLB checks. The
evidence is recorded in the [Stage 14.5D revalidation audit](docs/audit-stages-consolidated.md).
It qualifies the QEMU virtio boundary only; native NIC/radio and physical-link
behavior remain hardware work.

## Stage 13.16 report-protocol HID: tablets, pens, touch, game controllers (2026-10-14)

`kernel/src/hid.rs` is a pure module (no allocation, no kernel state) that
host tests compile directly (`tests/hid_reports.rs`).

**`parse`.** It reads a HID report descriptor into a bounded `Layout`:
- the top-level application: mouse, pointer, pen, touch screen, touchpad,
  joystick or game pad;
- up to 48 input fields, each with its usage, report ID, bit offset and
  size, logical range, and whether it is relative.

It handles report IDs, usage ranges, push and pop, padding, and long
items. A malformed descriptor, or one with an application it doesn't
decode, is refused.

**`Decoder`.** It turns reports into `Input` events and keeps the state it
needs to report only changes:
- relative and absolute pointer motion;
- wheel;
- button edges;
- pen position, pressure and contact;
- per-contact touch, with each contact's lift;
- game-controller buttons, axes and hat.

Positions are scaled to 0..=32767 and pressure to 0..=1023.

**xHCI.**
- `parse_hid_interface` reads the report descriptor's length from the HID
  class descriptor.
- A HID interface that isn't a boot keyboard or mouse has its report
  descriptor fetched (GET_DESCRIPTOR, report type, addressed to the
  interface) and parsed. It is kept as an input device only if the layout
  decodes.
- Reports up to 64 bytes are copied.
- The `usb-input` worker builds one `Decoder` per report device when it
  starts, and maps decoded inputs to WovenInput events, still outside the
  controller lock.

**WovenInput and userspace.** `Scroll`, `PointerAbsolute`, `Touch`, `Pen`
and `GameController` are production events, encoded to userspace as input
kinds 9â€“13 and decoded by libwoven's `InputEvent`.

## Stage 13.15 WovenAudio playback for userspace (2026-10-14)

When bring-up starts `hda0`, it calls `hda::start_output("hda0")`.
1. **Output setup.** `prepare_output` finds the output route (converter,
   pin, amplifier), enables the pin, sets the amplifier to 75%, and
   allocates the playback stream's memory once: a buffer descriptor list
   and 16 DMA pages, 64 KiB in all.
2. **Interrupt.** The controller's MSI-X or MSI vector is bound through the
   driver manager.

**Syscalls.**
- **203 `AudioInfo`:** the output format, 48 kHz 16-bit stereo, and the
  largest buffer one play accepts.
- **204 `AudioPlay`:** needs the new `AudioPlayback` capability.
  1. It copies the PCM into kernel memory, because user pages may fault and
     the controller's IRQ lock must not be held across a fault.
  2. Only one playback runs at a time. The calling task registers as the
     vector's waiter.
  3. `begin_playback` fills the pages and the descriptor list, with
     interrupt-on-completion on the last entry only, and starts the stream
     with IOCE set. The caller then sleeps in `wait_for_event_until`.
  4. The stream's completion interrupt wakes it. It checks BCIS, stops the
     stream and clears its waiter registration.
  5. Every attempt is recorded in the security ledger (`Action::AudioPlay`).
- **205 `AudioSetVolume`:** needs `AudioPlayback`; sets the output
  amplifier gain, and 0 mutes.

libwoven exposes `audio_info`, `audio_play` and `audio_set_volume`.

**Capability.** `AudioPlayback` is capability 24 and permission 16
(`Permission::Audio`). It is in the system-service ceiling and in the user
ceiling, so the permission broker can grant it. Recording is not
implemented yet; it will use the existing broker-gated `Microphone`
capability.

## Stage 13.14 storage on NVMe and AHCI (2026-10-14)

The storage layer reaches its disk only through `block_io`:
- the block-I/O worker's `direct_primary`;
- the `PrimaryAta` handle, which storage uses for persistence, journals and
  swap;
- the FAT32 mount in `storage::mount_ata_root`.

`block_io` now has a selectable root device, `RootDevice::{Ata, Nvme,
Ahci}`, and every one of those paths goes through `block_io::with_root`.
The default stays ATA, so every existing configuration behaves as before.

A normal boot with no ATA volume mounted does this after driver bring-up:
1. `drivers::storage_root` picks the first ready `nvme0` or `ahci0`.
2. `block_io::select_root` makes it the root device.
3. The volume is mounted at `/mnt` through the same storage code, so
   persistence, the data journal and swap all run on the NVMe or AHCI disk.

`storage.rs` no longer refers to the ATA driver directly.

**Still ATA-only.** WovenFS (`wfs.rs`) remains on the ATA primary master.
Its `/wfs` VFS publication is serialized by a rank-5 mutation lock. Mount and
unmount invalidate cached descendants and stale open descriptions before a
fresh journal-replayed import; syscall paths fail closed while unpublished.
The authorized native adapter may still prepare a volume before publication.

## Stage 13.13 USB keyboards and mice (2026-10-14)

When bring-up starts `xhci0`, it calls `xhci::start_input("xhci0")`.

**Enumeration.** Every connected root port gets its own reset and slot.
`enumerate_hid` runs on the controller's active fields, which are then
moved into a per-device `InputDevice`. Each device keeps its own slot, EP0
ring, contexts, interrupt ring and report buffer, for up to four keyboards
and mice. Each one is switched to report-on-change (`SET_IDLE 0`) and has
one interrupt-IN transfer armed.

**Interrupts.**
- An MSI-X or MSI vector is bound through the driver manager.
- `interrupts::set_pci_vector_waiter` registers the `usb-input` worker as
  that vector's waiter. The generic PCI handler, after publishing the work
  flag, wakes the waiter through the scheduler event latch, so a wakeup
  cannot be lost.
- Interrupter 0 is enabled, moderated to one interrupt per millisecond.

**The worker.**
1. Under the controller lock, `service_inputs` clears IMAN.IP and drains
   the event ring. It never waits.
2. For each completed transfer it copies the report out and re-arms the
   transfer.
3. Only after unlocking does the worker decode and publish into
   WovenInput: keyboard key-down edges (`keyboard_presses`, with shift and
   held-key suppression) and mouse motion and buttons
   (`decode_boot_mouse`).
4. It then sleeps in `wait_for_event` until the vector fires again.

USB input reaches the same queue as PS/2, so the kernel shell, `/bin/sh`
and the desktop all receive it.

**Two fixes found by this stage.**
- **MSI-X never delivered.** `msix::enable_owned` enabled the function with
  Function Mask asserted and never cleared it. Function Mask masks every
  vector, so no MSI-X interrupt could ever be delivered on any device. It
  is now cleared, and the write verified, as the last activation step.
- **The USB mouse Y axis was inverted.** HID boot mice report Y positive
  downwards, which is already WovenInput's orientation; only PS/2 reports
  up as positive.

## Stage 13.12 boot-time driver bring-up (2026-10-14)

Until this stage, every Stage 13 driver (`driver.rs`, `nvme.rs`, `ahci.rs`,
`xhci.rs`, `hda.rs`) was compiled only into its own test feature: a normal
boot contained none of them. `drivers.rs` now runs in every normal boot,
after storage and networking:
1. It registers a class matcher for NVMe, AHCI, xHCI and HDA with the
   WovenDriver manager.
2. For each driver, it finds the first PCI function the manager matches to
   it.
3. It binds that function through `driver::bind_pci`. Binding claims the
   function and its programmed MMIO BARs for the boot owner token.
4. Only then does it call the driver's `init_at(device)`, with the function
   re-read after admission because admission may move BARs.

Initialization failure unbinds again (`unbind_pci_resources`), so a failed
controller never keeps ownership. A missing controller is logged as absent.
Neither stops the boot. Ready drivers are added to the device registry
(`nvme0`, `ahci0`, `xhci0`, `hda0`) and listed by the shell's `devices`.

**Rules for this code:**
- A normal build compiles only the driver code it uses. The PCI hotplug
  functions stay gated to the Stage 13 tests.
- Each driver module carries one `expect(dead_code, reason = â€¦)`, active
  only in production and in `stage13-12-test`. The reason names the APIs
  still waiting for production callers. The later Stage 13 closure steps
  (USB input, audio) remove it by giving those APIs callers.
- Shared PCI/paging items that the live binding path now uses had their
  dead-code expectations narrowed so they do not apply in those builds.

**Limits.**
- One function per driver: a second controller of the same class stays
  unbound.
- The controllers still poll synchronously under their own locks during
  initialization.
- Storage is not yet mounted from NVMe or AHCI.
- USB devices are not yet read, and audio is not yet exposed to userspace.

## Stage 13.4â€“13.7 revalidation (2026-10-14)

The ordered AHCI/SATA, xHCI core, USB HID, and WovenInput software gates pass
on 1/2/4 CPUs in the current tree. Their evidence is recorded in the
[Stage 13.4â€“13.7 validation audit](docs/audit-stages-consolidated.md).
The QEMU fixtures validate bounded ownership and protocol contracts; they do
not substitute for native-controller or physical USB interoperability. USB
mouse report decoding is still outside the accepted keyboard-only fixture.

The bounded USB boot-mouse decoder now exists in `xhci.rs`; it emits WovenInput
motion and button-edge events from copied report bytes and carries button state
explicitly between reports. It is self-tested and does not yet claim a live
xHCI mouse polling worker or physical-device qualification. See the Stage 13.6
USB mouse decoder audit.

## Stage 15 bounded WovenFS VFS write-through (2026-10-14)

The `/wfs/<name>` namespace has one production mutation authority:
`task::open_current`, `task::write_current`, `task::mkdir_path`,
`task::rename_current`, and `task::unlink_current` route
through native WovenFS. Creation is durable before a read-only VFS node is
published; writes commit the native transaction before updating the VFS cache
or shared descriptor offset; unlink follows durable-first ordering. The
adapter validates nested native paths and rejects traversal, empty components,
NULs, and prefix confusion.

Native mutations serialize their complete read-modify-write operation with a
rank-5 `PreemptMutex`, below the ATA device lock at rank 10. No IRQ-disabled
guard spans device I/O. Directory creation, empty-directory removal, and
rename update parent entries transactionally; ATA publication recursively
imports validated nested directories with bounded depth and visited-inode
tracking; `Volume::check()` applies the same complete-tree integrity boundary.
Mounted system policy and physical flash qualification remain open. See the Stage 15
write-through audit.

## Stage 11.2 scheduler-backed thread ownership (2026-10-14)

ASLR geometry correction (2026-10-20): the graph-loader keeps the independent
random draws but passes them through a checked reservation boundary. The full
sixteen-slot mmap arena, optional static-TLS/template reservation, one-page
guard and user stack are disjoint; a requested stack draw is raised above the
arena when necessary. The Stage `11.2-aslr` forced stack=16/mmap=0 regression
now faults on the guard and passes on 1/2/4 CPUs. The planner is internal and
does not change the userspace ABI. Broader ASLR entropy/arena qualification
and physical-hardware validation remain release work.

Bounded main-image local-exec TLS accepted (2026-10-16): `InitialTls` records a checked
Variant-II layout and an independently tracked RO/NX post-relocation template.
Mutable main/child storage contains a mapped FS:0 self pointer; fresh threads
receive initialized template state, not mutated main-thread bytes. Parser
backing/congruence, clone, logical permission and teardown accounting are
updated together. Expanded compiler fork/cancel/capacity/owner-exit and biased
pointer-initializer execution passes on every online CPU in 1/2/4-CPU boots;
full session 37505 passed with 515 unchanged inputs, seventeen feature lints,
nine focused and twenty-one inherited boots, plus sixty stress logs. See the
implementation audit for scope: pure-BSS runtime remains open, as does the
separate production-ASLR arena-overlap finding. Later older TLS-pending
statements in this section describe the pre-correction baseline.

Compiler shared-PIE correction accepted (2026-10-15): ELF parsing explicitly
accepts a single inert null DT_DEBUG entry, rejecting nonzero addresses and
duplicates. The new opt-in compiler gate executes exact signed Rust/LLD output
through the graph loader before requiring every original Stage 11.5 check.
Session 73596 passed full host/lint and inherited 1/2/4-CPU preservation with
all 505 inputs unchanged. The later shared-library TLS correction below closes
the compiler-fixture TLS gate; see the compiler-PIE audit for the historical
scope.

Shared-library TLS correction accepted (2026-10-20): the graph loader now
recognizes compiler-emitted TLS symbols by `STT_TLS` and their ordinary
non-undefined section index, rather than treating `SHN_COMMON` as a TLS
section. Combined main/library PT_TLS planning, immutable templates, per-
process TCB initialization, TPOFF64 relocation, and exact signed Rust/LLD
compiler fixtures pass on 1/2/4 CPUs. The task stack reservation is 256 KiB
to cover the compiler-enabled kernel path. Physical hardware qualification is
still a later production stage.

Accepted loader placement correction (2026-10-15): the validated ELF kind now
selects main-image placement. ET_EXEC ignores an offered ASLR delta; ET_DYN
retains it. The signed graph loader, dependency ownership and library windows
are preserved. A nonzero-candidate compiled static fixture and a biased PIC
probe pass on 1/2/4 CPUs. Full host/lint, exact Stage 10.7 and Stage 10.8â€“11.5
preservation passed in session 62865, exit 0, with all 500 inputs unchanged.
See the executable-placement audit and Claude's source policy `6f02202d`.
The compiler TLS gate was subsequently closed by the shared-library TLS
correction recorded above; this historical placement entry alone did not
qualify a new release or full stage.

Accepted foundation/status corrections (2026-10-15): user-mapping ancestors
are explicitly writable while leaves retain W^X/RELRO/COW policy, including
read-only COW/file mappings; this prevents sole-owner COW refaults. Process wait
checks parent ownership before scheduler liveness, without nesting those locks,
and rechecks in the original reap transaction. Process wait and thread join
share a zero-extending 32-bit exit payload encoder, separate from 64-bit errors.
The real libwoven fixture catches these boundaries and preserves original
Stage 11.2 assertions. All three bounded corrections are accepted in prerequisite
order on the unchanged 500-input baseline, with full Stage 10.7/10.8â€“11.5
preservation on 1/2/4 CPUs and warning-denying host/fixture/kernel QA. See the
three dedicated audits and combined evidence. None is a release/hardware
qualification or complete compiler-TLS/loader interoperability.

Earlier-stage IPC follow-up (2026-10-15, bounded correction accepted): `HandleSpace`
now skips empty slots whose preserved 24-bit generation is already maximum.
The last live generation remains valid until close. The existing state guard,
16-slot ordinary capacity, 32-bit encoding, metadata and five installation
paths are unchanged. Actual-source boundary probes, production rollback/teardown
and inherited runtime preservation pass on the frozen baseline; see the
handle-generation audit. This is separate from thread-record retirement.
Durable host coverage is now part of `cargo test`; a `qemu-test`-guarded
post-SMP probe exercises real installation rollback, queued escrow, waiters,
lineage and frame cleanup. Its final revised full acceptance chain exited 0,
including full Stage 10.7 and Stage 10.8â€“11.5 matrices on 1/2/4 CPUs. All 488
frozen inputs matched at that increment's acceptance. Signed-status encoding
has since passed its own separate 500-input acceptance;
full runtime, release and hardware production closure remain open.

The registry now reserves generation `u32::MAX` as a permanent exhaustion
sentinel: a free record at that value is never allocated again. All three
release paths retain their existing checked generation advancement under the
rank-10 registry guard. The accepted bounded correction has five actual-source
host regressions plus full Stage 10.7 and Stage 10.8â€“11.5 preservation on
1/2/4 CPUs with unchanged frozen sources; see the thread-generation audit.
The exit-result ABI collision has since been corrected and accepted separately;
compiler-compatible TLS remains open.

`TaskControlBlock` carries both the stable process owner and an optional
generation-safe thread ID. User-thread creation maps a private NX stack and a
private initialization from the immutable main-image TLS template into the existing
address space; it never retains an unchecked userspace pointer. Thread exit
publishes registry termination only after the scheduler completes the stack
hand-off, so join can safely reclaim those mappings. Join waiters use the
scheduler's latched event path, and process termination retires ready/blocked
siblings immediately while running siblings receive a safe-point termination
request. `ThreadCancel` (ABI 136) applies the same two-state rule to an owned
sibling without killing its process: ready/blocked targets are marked dead and
the registry is transitioned under the scheduler lock, while running targets
carry the request through the existing post-switch finalizer. Both paths wake
the single join waiter and retain the generation-tagged record until join
reclaims the private mappings. The address space remains owned by the process
until its parent reaps the process. Process exit now retires unjoined sibling
threads, defers address-space destruction until all scheduler hand-offs are
complete, and reclaims generation-tagged thread records only after the parent
wait path releases the process-table lock. Shared libraries, symbol
resolution, dynamic TLS, and physical qualification remain outside this
bounded increment.

## Stage 11.5 bounded symbol relocation (2026-10-14; scope groundwork 2026-10-15)

`elf::DynamicImage` validates a SysV hash table, dynamic symbol table, and
string table within bounded file-backed load segments. `userspace::load_elf`
applies `R_X86_64_RELATIVE` and locally defined `R_X86_64_64`,
`R_X86_64_GLOB_DAT`, and `R_X86_64_JUMP_SLOT` entries before W^X/RELRO
finalization. The subsequent signed-library integration in
`userspace/linker.rs` loads a breadth-first graph of at most four verified WAPP
dependencies from `/lib`, checks SONAME identity, maps each into a separate
window, applies cross-image relocations, and protects every RELRO range.
`elf::SymbolScope` preserves discovery order and supports transitive cycles.
The process owns all mappings; an unpublished `Pending` guard rolls back all
failures, including argv setup. Compact page-range metadata retains forty
mapping slots within the existing fork stack budget. Root destruction walks
every empty private user-table branch and refuses copied kernel branches.
`DT_REL` tables are now accepted as a separate bounded eager path: the loader
validates `DT_RELSZ`/`DT_RELENT`, reads the signed addend from the already
mapped relocation target and applies it before W^X/RELRO. The Stage 11.5
acceptance fixture covers relocation, Ring-3 execution and owner teardown on
1/2/4 CPUs. Weak default-visible definitions are accepted as fallback exports,
strong definitions win interposition, and undefined weak imports resolve to
zero. `DT_JMPREL` is accepted as bounded file-backed RELA or legacy REL
and is applied eagerly before W^X/RELRO; see
`docs/audit-stages-consolidated.md` and
`docs/audit-stages-consolidated.md`. Weak definitions and SysV versioned
symbol tables (`DT_VERSYM`, `DT_VERDEF` and `DT_VERNEED`) are bounded and
resolved by exact requested version; see
`docs/audit-stages-consolidated.md`. Lifecycle metadata
(`DT_INIT`, `DT_FINI`, and both array forms) is bounded and executable-range
validated. Constructors run before initial Ring-3 entry, while destructors
run in reverse dependency order through a mapped RX trampoline; teardown uses
a prebuilt stack return chain and transactional process-table/paging lock
ordering. Writable PLT `JUMP_SLOT` entries use bounded loader-generated RX
resolver stubs and a checked kernel first-call patch path; RELRO slots remain
eager. See `docs/audit-stages-consolidated.md` and
`docs/audit-stages-consolidated.md`. The loader now constructs a
combined immutable PT_TLS template for the main image and shared libraries,
then gives each process a fresh mutable TLS/TCB copy and resolves shared TLS
relocations. The compiler-fixture gate remains open pending its preserved
feature-specific stack-canary failure.

The historical main-image TLS mapping was not a compiler-compatible TCB:
the real Rust local-exec probe expects a self-pointer at `FS:0` and aligned
template bytes immediately below it. The existing page-end FS base has neither;
new-thread cloning also copied mutable main-thread state. The correction and
full bounded compiler local-exec acceptance are recorded above and in the
compiler-TLS implementation audit. Do not infer broader TLS-model acceptance.

The symbol resolver and the production loader now share
`Image::defined_symbol_value`: ELF symbol values receive the image load bias
before validation against relocated segments, and the entire declared symbol
range must fit. Local relocation can reference private definitions; scope
exports require global binding, default/protected visibility and ordinary
NOTYPE/OBJECT/FUNC semantics. Undefined, TLS, IFUNC and reserved section values
cannot become ordinary relocation addresses. Dependency scope validation
preserves `DT_NEEDED` order, and resolution requires a complete validated scope.
Parser-backed regression fixtures live in `tests/elf_scope.rs`.

Debug builds optimize the `sha2` dependency (`profile.dev.package.sha2`) so
hashing the complete debug kernel ELF does not dominate emulated boot. Full
kernel measurement and the SHA known-answer check remain enabled.

## Stage 13.2 PCI power-state ownership (2026-10-13)

`hal::pci::suspend_owned` and `resume_owned` operate only on a validated,
generation-owned function. Suspend first disables and verifies I/O, memory,
and bus-master decoding. If the discovered PCI capability list contains Power
Management, the exact low-half PMCSR value is checkpointed and the function is
transitioned to D3hot; resume transitions the same capability to D0 before
restoring the command checkpoint. PMCSR writes preserve the upper half of the
containing configuration dword, preventing adjacent PM capability fields from
being clobbered. A failed transition attempts the safe rollback state and
returns an error, while the driver manager retains the checkpoint until the
binding state publication succeeds. Interrupt-active bindings cannot suspend.

This is a software ownership and configuration-state boundary. Device reset,
D3cold, platform wake signaling, and physical device suspend/resume still
require device-specific drivers and hardware qualification. See the
[Stage 13.2 suspend/resume audit](docs/audit-stages-consolidated.md).

The PCI hotplug coordinator compares both BDF and the full driver-relevant
identity (vendor/device, revision, class, subclass, and programming
interface). A same-slot replacement detected before old-owner quiescence fails
closed before any teardown configuration write; a platform hotplug boundary
must establish ordering before the replacement can be processed. Unbound
persistent BAR reservations are released before topology invalidation;
release failures leave the removal retryable. See the [hotplug insertion audit](docs/audit-stages-consolidated.md).

## Stage 13.2 PCI BAR admission lifetime (2026-10-13)

Persistent BAR reservation state is released only after BAR clear/read-back,
published-inventory refresh, and bridge-window rerouting succeed. The binding
remains in `Unbinding` when any phase fails, so no allocator range can be reused
while hardware or forwarding state is uncertain. See the [automatic BAR
admission audit](docs/audit-stages-consolidated.md).

## Stage 13.2 verified MSI activation (2026-10-13)

Owner-bound MSI activation keeps the vector and interrupt-mode claims private
until the capability header, message address/data, and final enable state all
read back as expected. The original capability image is retained with MSI
disabled during recovery; if restoration cannot be verified, the concrete
lease remains retained for ordered teardown. Disablement likewise verifies the
final disabled header before releasing mode and vector ownership. Physical
APIC delivery still requires hardware qualification. MSI-X activation follows
the same ownership boundary, asserting Function Mask before table MMIO and
retaining the lease if cleanup fails. See the [MSI rollback audit](docs/audit-stages-consolidated.md).

The Stage 13.2 preservation probe keeps hardware qualification conditional:
it scans the discovered QEMU fixtures without creating shared mappings, and
only attempts generic interrupt activation when a capability and the required
BAR are admissible. Its disposable NVMe/e1000 fixtures exercise inventory and
fail-closed admission; the QEMU virtio fixture now qualifies MSI-X activation
and teardown, including the high-MMIO fallback mapping. Physical vector
delivery and PCIe hardware lifecycle qualification remain open.

## Stage 13.3 NVMe queue and failure lifecycle (2026-10-13)

The NVMe driver owns each queue, identify buffer, and transfer buffer through
an owning `DmaPage`; dropping any partially initialized controller therefore
returns its physical frames to the allocator. Initialization also snapshots the
PCI command register and uses a rollback guard that disables the controller
with bounded readiness polling before restoring the exact command value. This
keeps failed discovery from leaking DMA memory or leaving bus mastering active.

The accepted gate is limited to the disposable QEMU controller and synchronous
polling I/O. Interrupt completion, controller reset/recovery, hotplug removal,
multi-namespace policy, and physical NVMe qualification remain open. See the
[Stage 13.3 NVMe audit](docs/audit-stages-consolidated.md).

## Stage 13.8 HDA register addressing (2026-10-13)

HDA MMIO access splits each register address into its containing page and
offset before performing the volatile access. This is required for the reset,
command-ring, codec, stream, and mixer registers; mapping a page without
adding the offset silently aliases every register to the BAR base. The QEMU
HDA codec/topology/stream gate now passes on 1/2/4 CPUs. Physical codec and
interrupt/DMA recovery qualification remain open. See the
[HDA MMIO audit](docs/audit-stages-consolidated.md).

## Stage 14 later-feature lint coverage (2026-10-13)

PCI persistent BAR admission diagnostics are guarded across the complete later
feature family, not only the Stage 13 feature set. This keeps `-D warnings`
meaningful when Stage 14â€“20 code compiles the shared PCI module. The live
Stage 14.1â€“14.5D network matrices pass on 1/2/4 CPUs; hardware NIC and radio
qualification remains separate. See the [Stage 14 feature-gate audit](docs/audit-stages-consolidated.md).

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

## AX201 integrated 22000 activation boundary (2026-10-17)

The AX201 owner increment adds `Ax201Driver` in `wifi_firmware.rs`. It is a
single lifecycle owner for the validated API-77 runtime image, copied
LMAC/UMAC/paging DMA, context-info DMA, startup state, ALIVE transition and
stop teardown. The firmware input is an explicit named provider source whose
filename must be `iwlwifi-QuZ-a0-hr-b0-77.ucode`; the external bytes are copied
before the source may be released. `Ax201QueueOwner` allocates the 22000 MQ
RX free/used/status rings, command ring and one RX buffer per 512-entry MQ
free descriptor,
and startup refuses to proceed without attaching it. The concrete adapter
programs the RFH free/used/status queue bases, resets their device cursors, and
rings the MQ RX shadow write-index PRPH doorbell at `0x1c80` before
`CPU_INIT_RUN`.
On reclaim, every device-produced AX201 used completion is checked for a
bounded rbid before any RX copy, preserving the DMA ownership boundary for
malformed completions and teardown. The free ring uses 16-byte transfer
descriptors; the used ring uses 32-byte completion descriptors.
The command ring uses the 22000 `iwl_tfh_tfd` 256-byte stride (25 TB slots plus
header/padding), rather than a legacy 128-byte descriptor assumption.
The context-info command-queue size is encoded as `TFD_QUEUE_CB_SIZE`
(`ilog2(entries) - 3`, `2` for the 32-entry ring); it is kept distinct from the ring's descriptor
count used for indexing and bounds checks.
The owner also exposes a bounded four-outstanding host-command path: it
copies the 4-byte short or 8-byte wide header and payload into kernel-owned
DMA, writes one 10-byte transfer-buffer entry in the 256-byte TFD, uses
command queue 9, and retains that payload until a matching response sequence
retires it. This is transport plumbing only; the physical probe sends no
speculative firmware command before the device's real ALIVE is observed.
The TFD is emitted by a shared checked encoder that bounds the 25 transfer
buffers, rejects zero lengths and addresses outside the 36-bit transport
range, and is covered by the queue-owner self-test before DMA publication.
The physical probe is invoked only after the normal IDT/APIC boundary is
initialized; it enables PCI memory/bus mastering, binds the identity-specific
AX201 MSI vector, enables CSR RX causes, services the deferred IRQ boundary,
and orders CSR/MSI disable before restoring the PCI command register.
Once ALIVE is observed, the physical probe uses the API-defined statistics
command (`0x9c`) as a bounded command/response smoke test, with periodic
notifications disabled. It reports that result separately from radio scan and
association qualification.
The firmware owner also exposes typed scan-control boundaries for the API
configuration and abort commands and for scan-complete notifications. These
encode/parse checks are strict about group, command, version, UID, and payload
size. It also emits the explicit API-17 passive UMAC request for at most 67
channels and validates the variable-length per-iteration result records.
Direct-SSID active probing remains fail-closed until probe-frame and MAC/PHY
context setup are implemented; these boundaries do not claim radio results.
The scan session owner retains one active UID, requires that UID for abort and
unsolicited completion handling, and clears it only on a matching completion.
This is the cancellation/generation boundary for the scan command path.
The AX201 boundary also exposes a checked API-1 PHY context encoder and
submission method. Its 40-byte packed layout validates channel, band, and
width before DMA publication. It is deliberately not part of the physical
probe sequence yet: MAC context, binding context, command status, and
quiescent teardown must precede radio-command activation.
The boundary now also contains the fixed managed-station MAC record and API-2
binding record. Physical activation submits those records only after ALIVE and
accepts them only through the generic zero-status response parser; timeouts,
unsolicited packets, and nonzero firmware status do not advance the sequence.
Once all three contexts acknowledge successfully, the physical path submits a
bounded passive scan on channels 1/6/11 and routes only UID-matching
iteration/completion notifications to the scan owner. A timeout attempts the
matching abort before teardown; active/direct-SSID probing and association are
separate later boundaries.
Physical startup reads the Intel `CSR_HW_REV` register at BAR offset `0x28`
for context-info `version.mac_id`; the PCI revision byte is not substituted.
It also reads the six-byte CSR MAC identity from `0x380/0x384` for the
activation diagnostic, while leaving radio/network qualification separate.
The inventory-only probe reports this register as well, but keeps its
read-only/no-DMA boundary.
`Intel22000MmioIo` accepts the 22000 24-bit PRPH address field
needed by AX201's `UREG_CPU_INIT_RUN` (`0x00a05c44`). Stage 13.10AE/AF/AG
passes in the Stage 13.9 1/2/4-CPU matrix. This is still synthetic: the owner
has no normal-boot attachment, physical ALIVE, queue-doorbell, interrupt,
recovery or radio evidence.

The Intel AX201 identity `8086:A0F0` shares the Intel 22000 CSR, DMA
context-info, firmware-startup and deferred-interrupt contracts already
implemented for the AX200-family path, but uses a separate integrated
firmware profile and MSI admission function. AX201 admission is therefore
explicit and cannot fall through the AX200 identity check. Firmware loading,
physical ALIVE proof, real queue activation, teardown/recovery and radio
qualification remain separate gates.

The AX201 owner now also has a distinct bounded data-TX ring: 32 hardware
TFD slots and up to four owned encoded payloads. A payload is published only
after its queue ID is checked against Intel's management (5â€“8) or data
(10â€“30) DQA ranges, and is
retained until an exact queue/index completion is decoded. The driver exposes
this boundary is now exercised by the physical probe's post-ALIVE DQA and
queue-allocation handshake. A tested management-frame submission still
requires an explicit frame/peer policy and returned hardware TX evidence.

## Physical Wi-Fi firmware prerequisite (2026-09-24)

The selected machine's AX201 PCI identity (`8086:A0F0`) is represented in the
Wi-Fi binding table as `ActivationReady`. Classification records the exact
family and integrated `iwlwifi-QuZ-a0-hr-b0` API-77 profile. The shared
22000 CSR path accepts AX201, while the AX201-specific MSI contract remains
identity-specific and cannot reuse the AX200 binding by accident. No firmware
blob is embedded or loaded; native firmware transfer, DMA, interrupt,
teardown and RF work remain prerequisites for physical activation.
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

## Stage 1â€“5 data journal boundary

FAT32 persistence retains the original single-file `WDJ1` journal and adds a
separate bounded `WDJ2` batch journal. A batch contains at most sixteen file data
replacements. Prepared records carry the old bytes and committed records carry
the intended lengths/checksums; WMD1/WMD2 metadata intents are published before
data and removed or finalized with the batch. Mount recovery rolls back
incomplete or partially matching batches, removes stale metadata intents, and
retires only a fully matching commit. The batch API is
`storage::persist_paths_atomic`. This is bounded data-plus-sidecar recovery,
not a general multi-operation metadata transaction or physical power-loss
qualification.
[Stage status](stage-status.md) records which gates have actually passed.

## Repository map

The production user stack is 64 KiB plus a 4 KiB guard page. Older repository
map wording that says 8 KiB is historical and is superseded by
`userspace::UserStack::SIZE`.

| Files | Responsibility |
| --- | --- |
| `Cargo.toml`, `kernel/Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml` | Workspace, pinned toolchain, freestanding kernel artifact and feature selection |
| `build.rs`, `src/main.rs` | Package the kernel with the UEFI bootloader; expose the generated image path to harnesses |
| `kernel/src/main.rs`, `config.rs` | Initialization order, subsystem wiring, configuration bounds and boot acceptance orchestration |
| `hal/`, `ap_start.S`, `smp.rs`, `gdt.rs`, `interrupts.rs`, `pic.rs`, `timer.rs` | CPU/platform discovery, AP startup, descriptor tables, interrupts and timekeeping |
| `task.rs`, `userspace.rs`, `elf.rs`, `syscall.rs` | Scheduling, process lifecycle, Ring-3 images, ELF validation, scheduler-checked static kernel-stack canaries, and syscall dispatch |
| `memory.rs`, `paging.rs`, `heap.rs`, `swap.rs`, `file_frames.rs`, `file_mapping.rs`, `page_cache.rs` | Physical/virtual memory, allocation, backing storage and file mappings |
| `block.rs`, `block_cache.rs`, `block_io.rs`, `ata.rs`, `partition.rs`, `gpt.rs`, `storage.rs` | Block devices, request execution, caching, partitions and storage initialization |
| `vfs.rs`, `fat32.rs`, `async_file.rs` | Filesystem operations and positional asynchronous file requests |
| `async_op.rs`, `async_network.rs`, `irq_lock.rs` | Completion ownership, asynchronous sockets and interrupt-safe network/completion locks |
| `network.rs`, `virtio_net.rs` | smoltcp IPv4 sockets and the transitional VirtIO-net transport |
| `capability.rs`, `wovenguard.rs`, `audit.rs`, `permission.rs`, `secret_vault.rs`, `signed_app.rs`, `secure_boot.rs`, `boot_measurement.rs`, `entropy.rs` | Authority, domains, resource policy, runtime permission decisions, volatile WovenVault handles, signed applications, software signed-kernel handoff, boot measurement, security-ledger recording/read ABI, and entropy support |
| `ipc.rs`, `pipe.rs` | IPC objects, endpoint/message operations and byte pipes |
| `device.rs`, `keyboard.rs`, `mouse.rs`, `serial.rs`, `console.rs`, `terminal.rs`, `shell.rs` | Device registry, input (keyboard and PS/2 mouse both decode and publish at IRQ time), diagnostics and command interfaces |
| `graphics.rs`, `gui.rs` | Kernel colour definitions and legacy framebuffer/GUI support; the userspace WovenGraphics abstraction lives in `libwoven::graphics` and feeds the existing compositor surface ABI. The userspace renderer also exposes bounded rounded panels, alpha-aware image blits, `Path` geometry with clipped opaque/alpha strokes and fills plus four-segment quadratic flattening, bounded eight-contour `CompoundPath` even-odd fills and strokes with integer 4x4 antialiasing, clipped vertical and horizontal gradients, deterministic signed-16.16 affine `Transform` values for clipped transformed rectangles, a shared-glyph `Font`/`TextMetrics` provider for bounded layout, wrapping and integer scaling, and a borrowed `TrueTypeFace` metadata/cmap/advance-metrics boundary with BMP format-4 and non-BMP format-12 lookup, bounded `fvar` axis metadata, validated borrowed `gvar` glyph-payload slices, simple and XY/transform/point-matched composite `glyf`/`loca` outlines, compound-path assembly, antialiased glyph raster entry point, advance-based text layout with optional legacy kerning, bounded character and word-level text-box line breaking, explicit LTR/RTL placement and shared bitmap fallback; `gvar` tuple delta application, hint execution, modern shaping, bidi and scripts remain open |
| `rtc.rs` | CMOS real-time clock (UTC), behind the `ClockRead` syscall |
| `font.rs`, `display.rs` | 5Ã—7 ASCII font (also compiled into libwoven); `Surface` (opaque `blit`, `blit_region` for exposed-region redraw, uniform-opacity `blend`, `write_pixels` for client uploads), bounded `Damage` (regions that form exactly a rectangle merge; a full list collapses to one bounding box) and the single format-resolving presentation path (`present_scaled`, integer display scale, each row converted once and copied to the physical rows it covers; `present` is scale 1; `Surface::scaled_row` serves the virtio-gpu heads) |
| `user_graphics.rs`, `compositor.rs` | Owner-tagged, generation-checked client surfaces (syscalls 98â€“101, 104), up to 1920Ã—1200 within a 4 MB per-process and 16 MB system pixel budget, behind a preemption lock (not interrupts-off) with allocation and freeing outside it and the kernel-held compositor surface they are blitted onto (102â€“103); `blit` snapshots under the registry lock and releases it before taking the compositor lock, so the two rank-10 locks never nest. Presentation borrows the framebuffer through `terminal::with_framebuffer` (lock order compositor â†’ terminal), and display ownership suppresses terminal rendering while the compositor presents. Userspace reaches the compositor only with `DisplayIo` and the single-holder lease (syscalls 105â€“106), released on exit; every client token is bound to the calling process before the registry sees it. A presenting holder also takes input focus: `woven_input` routes events to its private bounded queue (syscall 107, `InputIo`) until it exits; syscall 108 sleeps for the next event with a single registered waiter woken by `publish` after `FOCUS` is released (lock order scheduler â†’ compositor â†’ FOCUS). A client may share one of its surfaces with one process (syscall 109); the lease holder composites it with syscall 110, authorised by the slot's grant rather than the token. Syscall 111 sleeps over input and the pid mailbox at once; `ipc::send` wakes the mailbox's single waiter after releasing the IPC lock. Clients declare damage on their surfaces (syscall 112); the holder sleeps on it via `WaitAny` and copies only the damaged region (syscall 113). The boot compositor is sized to the framebuffer (clamped to 1920Ã—1200), and every framebuffer writer, including the boot `Console`, goes through the terminal lock. Stacking, raise and per-window focus are window-manager policy in userspace (Stage 19.11), built on the lease, focus, `WaitAny`, shared surfaces and IPC |
| `panic.rs`, `benchmark.rs` | Failure reporting and existing benchmark support |
| `libwoven/`, `apps/hello`, `apps/hello-gui`, `apps/woven-shell` | The userspace API (one thin wrapper per syscall, plus the typed `graphics::{Color, Rect, Renderer}` WovenGraphics boundary, `Canvas` software rendering, caller-owned `DepthBuffer`/`Vertex3` software 3D triangles, the shared kernel font, and a fixed-point `Spring` for animation, and `accounts`, local accounts held as a salt and `PBKDF2-HMAC-SHA256(PIN, salt, PBKDF2_ITERATIONS)`) and the Ring-3 programs. Each app is its own crate with a large-code-model `link.x` (whose `.l*` sections must be placed explicitly) and a `build.rs` that relinks when it changes. `woven-shell` is the desktop, with four workspaces (per-workspace stacks, a taskbar switcher, F3/F4): it holds the compositor lease and recomposes exposed rectangles from its own software layers and clients' shared surfaces (region blits, syscalls 120â€“121), hosting applications over an IPC protocol (`WOPN`, `TRAY`, `NOTE`, `WCLS`, and `WSIZ`/`WRSZ` for resizing; `libwoven::layout` tiles them); shadows and other translucent layers use region blends (syscalls 200â€“201), and text, borders and the pointer are overlays copied with a keyed blit that skips the transparent key (syscall 202), so nothing is read back from the screen. With `--accounts` it starts at a sign-in screen, and its lock screen asks for the signed-in account's PIN. A user stack is 64 KiB (`UserStack::SIZE`), so the shell keeps its tile buffer and desktop state in statics (.bss), never in `main`'s frame. The kernel's feature-gated VirtIO-GPU test path separately uses a bounded `VirglStream` encoder for checked VIRGL command framing; it is transport validation only until a visible GPU frame is accepted |
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
through 8 MiB; when the memory map can afford the complete 8 MiB bound, the
full bound is reserved so the boot compositor's large surfaces cannot fail on
the retained low-memory regression fixtures. The allocator retains alignment padding with each allocation
and coalesces freed intervals. Kernel range mapping rolls back newly mapped
pages on a partial failure. Runtime growth maps 256 KiB chunks only after
releasing the heap guard; the boundary is published under that guard after
mapping succeeds. A rank-free, IRQ-enabled caller can grow synchronously on
allocation failure. BSP idle maintains mapped headroom when free space falls
below 1 MiB so guarded callers usually stay on the allocation-only path.
The growth window remains in the P4 slot already shared by user address
spaces. See the [growth audit](docs/audit-stages-consolidated.md).
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
The historical Stage 10.8 completion-port gate is preserved in
`audit-stages-consolidated.md`; its obsolete runner is no longer part of the
active repository surface.
Timers, event objects, IPC completion integration and per-thread cleanup remain
later roadmap work.

## Stage 10.9 timers and events

`async_events.rs` owns bounded generation-tagged timer and manual-reset event
slots. Timer deadlines use the monotonic tick source and are pumped by an
event-driven worker. Syscalls 87â€“94 expose create, wait, set, close and
sleep-until operations; timer and event waits produce the same generic async
completion records as other operations. Owner teardown cancels outstanding
waiters and releases every slot.

## Stage 13.10 Bluetooth architecture

Roadmap Stage 13.10 owns Bluetooth. Historical source/audit labels 13.10Aâ€“AC associated with AX200 are retained as historical Wi-Fi substage names under roadmap Stage 13.9 and must not be interpreted as Bluetooth milestones.

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
## Unified timer/event waits

Stage 19.8 keeps syscall 111 stable for input, mailbox, and compositor damage.
Syscall 206 (`WaitAnyObjects`) adds a separate bounded ABI for up to eight
owner- and generation-checked timer/event objects. The kernel copies the source
array before registration, uses task wakeups rather than polling, and clears
registrations on every return path. Manual-reset events support four bounded
task waiters and broadcast wakeups; one-shot timers retain one consuming task
waiter so expiry is not duplicated. See [the ABI contract](wait-any-objects-abi.md)
and [the Stage 19.8 audit](docs/audit-stages-consolidated.md).

## Stage 25.1 ACPI power capability boundary

`kernel/src/hal/acpi.rs` retains the discovered `_S5` types and now records
optional `_S3` sleep types in `Power`. Syscall 235 reports fixed capability
bytes for power-off, suspend, and restart; it requires `PowerControl`, audits
the query, and copies only after validating the userspace destination.
`libwoven::power_capabilities` is the corresponding typed wrapper. This ABI
does not initiate a power transition: S3 entry/restore, driver quiescing,
wake-source policy, and hardware qualification remain separate work.

## Stage 25.2 CPU idle capability boundary

`kernel/src/hal/cpu.rs` reads only CPUID leaves to report `HLT`, optional
`MONITOR/MWAIT`, and invariant-TSC support. Syscall 236 applies the
`PowerControl` authority boundary, audits the read, validates the destination,
and copies three capability bytes without executing an idle instruction.
`libwoven::cpu_idle_capabilities` is the typed wrapper. The current scheduler
continues to use the established interrupt-safe `STI+HLT` sequence; no MWAIT
or frequency MSR is enabled by this ABI.

## Stage 25.3 CPU frequency inventory boundary

`kernel/src/hal/cpu.rs` reads CPUID leaf `0x16` only when the basic CPUID
maximum leaf advertises it. Syscall 237 returns an explicit availability byte
followed by base, maximum, and bus MHz values, after the `PowerControl`
authority and userspace-copy checks. `libwoven::cpu_frequency_info` decodes
the bounded result. This is inventory only: it does not write P-state or
frequency-control MSRs.

## Stage 25.4 bounded suspend preparation

`kernel/src/driver.rs` provides `suspend_all` and `resume_all` as a bounded
transaction around the existing per-driver suspend/resume hooks. The registry
lock is released before any driver callback runs. Suspension follows registry
order; refusal restores the already-suspended prefix in reverse order, and a
successful preparation is later aborted in reverse order as well. This keeps
hardware callbacks outside the registry lock and makes rollback deterministic.

`kernel/src/power.rs` exposes the internal `prepare_suspend` and
`abort_suspend` boundary. Preparation requires ACPI mode, a PM1a control
block, and discovered `_S3` types. It returns a private token rather than
claiming that sleep occurred. S3 register writes, wake-vector installation,
CPU coordination, wake-source policy, and a userspace suspend ABI remain
separate prerequisites.

## Stage 25.5 ACPI FACS wake-capability boundary

`hal::acpi` follows the FADT firmware-control pointer and retains a `Facs`
record only after validating its firmware memory range, `FACS` signature, and
minimum 64-byte length. The 64-bit X_FACS pointer is preferred over the legacy
32-bit pointer. The record keeps the version and both waking-vector fields for
the future transition path.

Syscall 238 and `libwoven::suspend_resume_capabilities` expose only five
bounded bytes: S3 advertised, FACS valid, 32-bit vector nonzero, 64-bit vector
nonzero, and FACS version. The query requires `PowerControl`, validates the
userspace destination, and performs no firmware or PM1 writes. Wake-trampoline
allocation, FACS vector ownership/installation, CPU save/restore, and S3 entry
remain separate prerequisites.

## Stage 25.6 low-memory wake-page reservation

`smp::prepare` reserves the existing AP startup page and a second distinct
usable page below 640 KiB before the frame allocator starts. The allocator
already excludes low memory, so both reservations remain firmware-safe and
cannot be handed to unrelated allocations. If no second suitable page exists,
`smp::wake_page` reports unavailable instead of aliasing the AP page.

`power::prepare_suspend` requires this reservation and carries its physical
address in the private preparation token. The page is not executable yet:
trampoline construction, FACS vector ownership, CPU-state preservation, and
the S3 transition remain later boundaries.

## Stage 25.7 wake-trampoline staging

`smp::install_wake_trampoline` reuses the bounded `ap_start.S` real-mode to
long-mode image. It clones the current kernel root, installs a private
identity map for the low transition, patches the reserved wake page, and
points the image at a dedicated aligned stack and wake entry. The returned
`WakeTrampoline` owns the three allocated page-table frames; release attempts
all three frames rather than short-circuiting teardown.

`power::prepare_suspend` stages this image only after driver quiescence and
rolls back drivers if staging fails. No FACS write occurs in this boundary.
The wake entry currently reinitializes GDT/interrupt foundations and parks;
register/CPU-state restore, secondary-CPU restart, FACS vector installation,
and S3 entry remain required before exposing a suspend action.

## Stage 25.8 CPU resume-context capture

`power::CpuResumeContext` is kernel-owned preparation state containing CR0,
CR3, CR4, EFER, RFLAGS, RSP, and the base/limit pairs for the GDT and IDT.
`capture_cpu_resume_context` reads the architectural state with the existing
privileged register access and minimal inline assembly for RFLAGS, SGDT, SIDT,
and RSP. `valid` rejects non-long-mode paging, an unaligned CR3, non-canonical
addresses, empty descriptor limits, or invalid required flags before a suspend
token is returned.

The `stage25-8-test` feature exercises the capture and validation path during
boot on 1/2/4 CPUs. The token remains private to the power subsystem, and
abort validates it before releasing the wake trampoline and resuming drivers.
No register restore, FACS write, secondary-CPU handoff, S3 entry, or firmware
wake is implemented by this stage.

## Stage 25.9 transactional FACS wake-vector installation

`hal::acpi::install_facs_wake_vector` uses the discovery-validated direct
physical map to write the reserved low wake-page address to the fixed 32-bit
and 64-bit FACS waking-vector fields. It first copies the prior values into a
private `FacsWakeVector` owner token; `restore_facs_wake_vector` writes those
values back during abort. A small lock serializes firmware-field mutations,
and the fixed writes run with interrupts disabled. The power preparation
transaction installs the vector only after driver quiescence, trampoline
staging, and CPU-context validation; any failure releases the trampoline and
resumes drivers.

The `stage25-9-test` feature tests the patch/restore transaction and rejects a
non-32-bit target without touching firmware memory. The actual FACS write path
is therefore part of production suspend preparation, while no suspend action
is exposed yet. Secondary-CPU quiescence, CPU-state restoration, PM1 S3 entry,
firmware wake, and physical qualification remain later prerequisites.

## Stage 25.10 secondary-CPU suspend barrier

`smp::suspend_other_cpus` is BSP-only and snapshots the exact online mask
before requesting each online secondary CPU to evacuate scheduler-owned work
and reach the existing offline checkpoint. It records the completed secondary
mask in a private `CpuSuspendBarrier`; if any request fails, the completed
prefix is restored immediately. `resume_suspended_cpus` attempts all recorded
CPUs in reverse order and succeeds only when the original mask is restored.

`power::prepare_suspend` takes this barrier after context capture, driver
quiescence, wake-trampoline staging, and FACS vector installation. Its abort
path restores firmware state, releases the trampoline, re-onlines every
secondary CPU, and resumes drivers. The `stage25-10-test` feature exercises
the barrier and exact-mask rollback on 1/2/4 CPUs. This stage does not yet
restore registers, write PM1 S3 controls, or handle firmware wake.

## Stage 25.11 PM1 S3 request boundary

`power::enter_suspend` is the private transition boundary after preparation
has committed. It validates ACPI mode and `_S3`, reads the PM1 control words,
preserves unrelated bits, replaces only SLP_TYP, and sets SLP_EN on PM1a and
optional PM1b. The successful hardware path transfers control to firmware and
does not return. If firmware leaves the CPU running, the function calls the
existing abort transaction and returns `StillRunning`; a failed rollback is
reported as `RollbackFailed` rather than being hidden.

`s3_control_self_test` validates the bounded control-word encoding and rejects
an out-of-range sleep type without touching I/O ports. The `stage25-11-test`
QEMU gate therefore never requests a destructive sleep transition. CPU
register restoration, firmware wake completion, suspend policy/syscall
exposure, and physical qualification remain separate prerequisites.

## Stage 25.12 CPU resume-context restoration primitive

`restore_cpu_resume_context` is a non-returning, unsafe assembly boundary
whose caller must first validate the kernel-owned `CpuResumeContext`. With
interrupts disabled it restores CR0, CR4, CR3, and EFER, reconstructs packed
GDT/IDT descriptors from the fixed `#[repr(C)]` context, loads them, switches
to the saved RSP, restores RFLAGS, and jumps to a supplied kernel continuation.
The continuation is never reached through a Rust return from the stale
pre-suspend call frame.

The `stage25-12-test` path captures the live context, executes the full restore
sequence, and reaches a continuation that records the marker and exits QEMU.
This establishes the architectural primitive on 1/2/4 CPUs. A persistent
firmware-wake record, wake-entry integration, driver restoration, and physical
S3 qualification remain required before exposing suspend.

## Stage 25.13 persistent firmware-wake record integration

`PENDING_SUSPEND` is a single kernel-owned slot populated only after the
prepared token is refreshed with the context captured immediately before the
PM1 request. `enter_suspend` rejects a second pending transition and takes the
slot back for every pre-transition failure. After firmware wake,
`wake_entry` snapshots the stored context and invokes the Stage 25.12
non-returning restore primitive. Its continuation consumes the pending token
and calls `abort_suspend`, which restores the FACS vector, releases the wake
trampoline, re-onlines secondary CPUs, and resumes drivers.

The `stage25-13-test` gate checks the empty-record invariant and canonical
wake/continuation wiring without issuing S3. Persistent record ownership and
rollback are implemented, but actual firmware wake and physical qualification
remain required.

## Stage 25.14 suspend admission boundary

Power-control action `2` is the suspend ABI entry point. It requires the same
`PowerControl` capability as poweroff and restart, records an audit event, and
returns error `4` until the kernel has a task-owned resume target for the
interrupted userspace syscall. The fail-closed path performs no suspend
preparation or hardware writes. `libwoven::suspend` exposes this result to the
desktop session without pretending that firmware S3 is production-ready.
## Stage 25.15 task-owned userspace resume target

The syscall assembly's `UserForkFrame` is the canonical saved register and
`iretq` layout. Action `2` validates that frame against the current task's
kernel stack before any suspend preparation, then stores its address in the
pending record. After wake, the continuation performs the complete owned
rollback and writes `RAX = 0` before jumping to `wovenhat_syscall_resume`,
which restores the frame and returns to the interrupted userspace caller.
Arbitrary pointers and missing records fail closed. This structural gate does
not certify real firmware S3 behavior.
## Stage 26.1 bounded accessibility core

`libwoven::accessibility` is the userspace-owned accessibility model. `Tree`
stores at most 128 nodes, each with a bounded UTF-8 label, stable `NodeId`,
role, bounds, parent, and focus/enabled state. Focus traversal is deterministic
and wraps across enabled focusable nodes; `Announcement` snapshots the focused
node for a screen reader without borrowing mutable UI state. `CaptionBuffer`
stores at most eight bounded captions. `Settings` centralizes high contrast,
reduced-motion duration policy, display scale, and magnification clamps.
Locale, translations, shaping, compositor integration, and device-specific
assistive technology remain separate prerequisites.

## Stage 28 disposable file-backed installation executor

`scripts/execute-file-install.py` is deliberately outside the production
installer path. It accepts only the validated plan JSON and an explicitly
supplied regular non-symlink file, binds the target identity to the file's
pre-install digest, and writes a deterministic mock filesystem payload through
same-directory temporary files and atomic replacement. A retained backup
allows verification or injected-failure rollback. Device paths, protected
identities, mounting, formatting, partitioning, bootloader installation, and
physical media are outside this boundary.

## Stage 28 disposable file-image filesystem formatter/verifier

`scripts/format-file-image-fs.py` consumes the layout builder's checksummed
metadata and requires exactly one aligned `data` region. Its bounded format is
FAT32-compatible with 512-byte sectors, one-sector clusters, 32 reserved
sectors, two FAT copies, FSInfo at sector 1, backup boot/FSInfo sectors at 6/7,
root cluster 2, and a deterministic `WOVENHAT` volume label. The formatter
constructs the complete region in memory and writes no bytes outside that
partition; verification rejects invalid geometry, bounds, copies, signatures,
or nondeterministic metadata.

The file-image transaction snapshots the selected region and restores it after
any injected pre-write, post-write, or post-verification failure. The input
must be an explicitly harness-created regular non-symlink file and all
protected/device identities are rejected. No mount, partition API, physical
disk, bootloader, or hardware qualification is involved.
# Stage 28 WovenInstaller shell boundary

## Stage 28 real UEFI artifact ingress

`scripts/extract-uefi-boot-artifact.py` is a read-only file-backed ingress
boundary for the real payload produced by the UEFI build. It checks the GPT
EFI System Partition, parses bounded FAT16/FAT32 directory chains, and emits
only the exact case-insensitive `EFI/BOOT/BOOTX64.EFI` file. It rejects
symlinks, protected/device-like paths, invalid geometry, loops, and payloads
over four MiB. The emitted regular file may be passed to
`scripts/execute-disk-install.py --artifact` for disposable transaction
coverage. It performs no mount, device I/O, firmware call, or hardware
qualification; the supervised service continues to fail closed for physical
installation.

`apps/woven-installer` is a freestanding Ring-3 presentation adapter for the
validated installer state model. Its input is a bounded semicolon-separated
snapshot with an explicit `validated=1` marker, a plan field, and the state
model's screen names. The default mode is text; `--gui` uses the existing
WOPN/shared-surface protocol to render a bounded resizable installer window
and handle pointer close events. It stores borrowed caller bytes only; its
sole external interaction is a bounded read-only IPC inventory request and it
has no access to installation authority. Parser
validation rejects unknown/duplicate fields, control characters, overlong
text, invalid progress, unvalidated plans, and screen-specific missing
prerequisites.

The binary's only output path is the existing bounded stdout/stderr API. The
progress and success screens are deliberately non-operative and print
`install_action=unavailable (fail-closed)`. The authority boundary is
`libwoven::installer_ipc` and the supervised `woven-installer --service`
process, published as `sys.storage.installer`; the GUI never gains storage
authority but may submit the bounded `Install` request after explicit
confirmation. The service validates bounded plans and, when a readable EFI
artifact and an eligible kernel-selected target are available, invokes the
kernel transaction for GPT layout, FAT32 formatting, EFI installation, and
provisioning. It fails closed when those prerequisites are absent. The
disposable Stage 28 gate verifies the transaction and rollback; physical
device selection and boot qualification remain separate Stage 29 gates.

# Stage 28 disposable acceptance composition

The end-to-end disposable-image gate lives in
`scripts/test-stage28-disposable-acceptance.py`. It imports the established
Stage 28 Python boundaries without modifying them: plan validation, GPT-like
file-image layout, FAT32-compatible data-region formatting, EFI artifact
preparation, and file-backed executor transactions. Every image is created as
a new bounded regular temporary file; no device path, mount, partitioning
command, or physical media is reachable.

The executor is intentionally tested on a copy because its contract atomically
replaces a whole file with a mock filesystem payload. The final image therefore
uses the layout/formatter/EFI path and records separate executor and final-image
digests. The harness verifies deterministic output and byte-for-byte rollback
after injected failures. It does not establish physical installation,
bootability, hardware qualification, or release certification.

The disposable harness releases the completed executor-success fixture before
creating the rollback fixture. This keeps the acceptance's same-directory
backup/staging transaction within its bounded temporary-storage budget on a
nearly-full development host without weakening any rollback or digest check.

# SMP scalability and supported resource floor

## Hybrid graphics GFX-01 boundary

The versioned `graphics_abi` contract is intentionally an adapter seam over
the established `display`, `compositor`, and `virtio_gpu` modules. It defines
typed surface/fence/output identities, bounded damage presentation, renderer
operations, display modes, and backend recovery errors. GFX-02 now provides a
bounded `CpuRenderer` implementation using the existing `display::Surface`
with generation-checked surface handles, geometry validation, compositing,
and fences. It does not allocate a second compositor or change scanout
ownership; the kernel compositor and capability boundary remain authoritative.
The affected Stage 20.14b graphics acceptance matrix and the GFX-02 1/2/4-CPU
probe are green. GPU backend migration and native display qualification remain
separate gates.

The production compositor path now accepts a single-output virtio-gpu as well
as the existing multi-head device. Multiple heads remain arranged side by
side at a bounded mode; a single head uses the reported primary mode. If GPU
probe or mode setup fails, the boot framebuffer remains the fallback. The
dedicated Stage 20.14S harness verifies a single-output frame capture on
1/2/4 CPUs with four quadrant-color samples. This closes the disposable-QEMU
single-output qualification while preserving the two-head Stage 20.14/20.14b
acceptance. It does not claim native GPU hardware certification, device-loss
recovery, or arbitrary graphics fault-injection coverage.

## Driver restart qualification boundary

The Stage 13.2 lifecycle gate proves that a discovered PCI function can be
bound, have its interrupt and BAR leases released in order, and be rebound
under a fresh owner transaction before a second teardown and generation-safe
rescan. This is the reusable teardown/rebind boundary. Automatic recovery
after a driver crash still needs a supervisor policy, device-specific reset
and replay rules, and hardware evidence.

The Stage 31 planner reports this as partial driver-crash coverage rather than
as a crash-recovery pass; the distinction is intentional.

## Service restart qualification boundary

The signed Stage 27.5 update-service gate now proves a full orderly restart
cycle: process teardown, service-registry cleanup, signed image reload, fresh
process publication, new capability discovery, request handling, and second
teardown. This is an acceptance boundary, not yet a production supervisor;
automatic crash detection, bounded retry/backoff, driver restart ownership,
and power-loss recovery remain separate designs.

## Input delivery and backpressure

`kernel/src/woven_input.rs` is the bounded fan-in queue for keyboard, pointer,
and future HID sources. IRQ publishers never allocate or sleep. Adjacent
relative pointer motion is coalesced with saturating deltas; when capacity is
exhausted, the oldest queued motion may be evicted to admit a critical event.
If the queue contains no evictable motion, publication fails and the bounded
drop counter records the loss. This policy protects keyboard and button
delivery without pretending that an unbounded input stream can be lossless.
The current live hardware path is PS/2; USB HID and virtio-input remain
separate driver/qualification stages.

The current static SMP control plane supports up to 16 online CPUs through
`kernel/src/smp.rs::MAX_CPUS`. A topology larger than that is deliberately
bounded and must be rejected or reported as truncated; it is not silently
treated as fully supported. Scheduler and process tables currently reserve
128 tasks and 128 processes, with 64 file descriptors per process, 256 IPC
objects, and 64 IPC handles per process. These are fixed, auditable capacities,
not an arbitrary-memory promise; dynamic per-CPU/object allocation remains a
future scalability stage.

The supported production memory floor is 4 GiB, exposed as
`kernel::config::MIN_SUPPORTED_RAM_BYTES`. Boot telemetry and the Windows
preflight report usable/discovered memory and whether it meets that floor.
The allocator still derives usable capacity from the bootloader memory map,
and the interactive QEMU launcher defaults to 4 GiB. Existing 256 MiB QEMU
fixtures are intentionally retained as low-memory regression tests and do not
claim production support below the floor.

The 1/2/4/8/16-CPU supported-suite matrix covers memory/boot, storage, network,
security, and shell paths. Under host TCG, the largest storage and shell
profiles require longer wall-clock harness budgets; those budgets only prevent
false host timeouts. Guest completion markers, SMP assertions, and exit status
remain mandatory. Failure injection, low-resource recovery, restart, sudden
shutdown, repeated-boot, and native hardware qualification are separate open
acceptance requirements.

Stage 31 process-crash containment is separately exercised by a feature-gated
Ring-3 probe on 1/2/4 CPUs. Its unmapped write must produce a user fault,
reap only the faulting process with status `-11`, and leave the kernel alive
to emit the QEMU success marker. This does not imply driver or service
supervision; those require their own owner and restart acceptance.

The Stage 31 packet-corruption boundary runs the Stage 14.2 ICMPv6 checksum
and NDP ingress-hardening probes through the live network QEMU harness on
1/2/4 CPUs. It proves deterministic malformed-checksum rejection, not a
randomized corruption campaign over every receive protocol.

`scripts/test-stage31-repeated-boots.py` is the repeatability boundary for
those suites. It serializes child harnesses because they use fixed ports and
shared target paths, snapshots each run before evaluating its exit status, and
fails closed on a timeout or non-zero result. It has bounded `--count` and
`--timeout` inputs, supports the five functional suites on 1/2/4/8/16 CPU
profiles, and supports the Stage 27.5 `recovery` and `update-service` suites
on their existing 1/2/4 CPU profiles. It does not claim a repeated-boot pass
until every requested iteration completes.
The host `tests/wovenfs_crash.rs` recovery model now adds a deterministic
xorshift schedule over 64 write-failure cut points, with a second remount
check after every sample. The test accepts only the complete pre-transaction
or complete post-transaction payload, preserving the journal's atomicity
contract while exercising a non-sequential failure order.

The durable recovery acceptance remains a separate executable QEMU boundary:
`scripts/test-stage27-5-recovery.py` provisions its signed recovery app and
verifies the recovery protocol on disposable media. Its current evidence
passes at 1/2/4 CPUs; it must not be conflated with native power-loss or
physical-media qualification.

For Stage 31 network-loss evidence, `scripts/test-network-qemu.py --link-loss`
adds a QMP control channel, issues `set_link` down/up for the named `net0`
device after the UDP proof, and then requires the existing TCP echo and final
guest markers. The QMP channel is opt-in and the normal network matrix is
unchanged; a QMP error, guest exit, timeout, or missing marker fails closed.

The FAT32 mount path repairs advisory FSINFO free-count and next-free hints
from an authoritative bounded FAT scan after journal replay. This is required
because an abrupt stop can persist a FAT allocation without its matching
FSINFO update. `fat32::repair_fs_info` rewrites both valid primary and backup
FSINFO sectors before VFS import; it never treats stale hints as authoritative.
The sudden-shutdown harness emits a kernel-owned injection boundary immediately
before the live mutation test and terminates QEMU at that marker, avoiding a
wall-clock race. Live mutation is skipped unless the storage layer reports an
actual mounted FAT32 volume, so service-only blank disks cannot be mistaken for
storage qualification.

Controller restart is a separate ownership boundary in `kernel/src/drivers.rs`.
NVMe, AHCI, xHCI, and HDA stop their DMA engines/rings before PCI BAR and
function leases are released, then rebind under a fresh owner token. If
quiescence fails, the concrete controller is restored to its global slot so its
DMA pages are not dropped while hardware may still reference them. xHCI first
stops its deferred enumerator and HID worker, removes the vector waiter only
after task death, and then halts the controller. The
`stage31-driver-restart-test` QEMU gate qualifies all four emulated controllers
on 1/2/4/8/16 CPUs; it is not a claim of generic crash recovery.

The seeded Stage 31 orchestrator includes `driver-restart` as a first-class
scenario alongside process, packet, shutdown, memory, service-restart, and
service-supervisor gates. The driver self-test uses a deterministic in-guest
Fisher-Yates shuffle (seed `0x51A7E`) over the four controller names and logs
the selected order. Its successful result proves reproducible order coverage
and child-gate integration. The same orchestrator also includes the existing
QEMU virtio link-loss gate, so network loss is part of the reproducible
eight-scenario schedule. These gates do not turn the bounded scenarios into an
arbitrary fault injector.

The current WovenShell qualification also treats client-surface release as a
composition boundary: when a client disappears, the shell removes its
owner-tagged window and recomposes the complete screen before presenting.
This avoids retaining stale client pixels in a partially damaged scanout
while preserving the existing damage-region path for live clients. The strict
desktop harness exercises this with Files and Terminal close/reopen journeys
on 1/2/4 CPUs; it remains QEMU evidence, not native GPU certification.
## Stage 29 hardware qualification report boundary

`scripts/qualify-hardware-report.py` is the report-only boundary for the
hardware compatibility laboratory. It consumes a versioned JSON manifest and
does not probe, mount, write, flash, or otherwise mutate a host. Every
observation identifies a category, device, execution environment, result, and
repository-relative evidence path. The classifier intentionally prevents
QEMU/emulated evidence from claiming Tier 1: native failures are Tier 3,
complete native coverage is Tier 1, and incomplete native coverage without a
failure is Tier 2. This separates evidence collection from policy and keeps
the release claim fail-closed until real hardware records exist.

## Graphics Stage 11 motion and pacing boundary (2026-10-25)

WovenShell's spring animation remains userspace-owned and advances only from
the event-driven `FrameClock`; it does not sleep while holding compositor or
IRQ locks. The Stage 20.7 motion oracle captures the pre-animation framebuffer
pixel, so it is independent of the separate wallpaper/projection renderer. It
validates monotonic alpha
samples and final settling, then verifies input routing, client teardown, and
display release. Stage 19.14 separately validates paced frames across integer
display scaling changes. This is a runtime acceptance of the software motion
and pacing boundary, not a claim of v-sync, low-power scheduling, or hardware
GPU acceleration; those remain separate qualifications.

The Stage 20.10 Quick Settings gate is the shell-facing accessibility seam for
the current motion policy. Its Reduced Motion row is exercised through the real
pointer path while the same gate checks bounded panel composition over both
wallpaper and a tiled client, scale changes, and teardown. The gate uses the
flat compatibility presentation so those checks remain independent of the
separate 3D projection path.

## Stage 32 production instrumentation compilation repair (2026-10-27)

The kernel entry crate exports the diagnostics, tracing, and profiler Rust
modules as instrumentation APIs; this adds no syscall or userspace authority.
Tracing counters/TSC helpers and profiler tables are compiled only for their
feature or host tests, and the profiler IRQ mutex only for active kernel
profiling. Boot self-test definitions have their own Stage 32 feature guards
(or host-test guards), matching the existing guarded call sites. Disabled
public operations retain their no-op behavior; production panic output and
all accepted record formats remain unchanged. No lint allowances were added.

## Desktop responsiveness and input routing repair (2026-10-27)

WovenShell processes at most 32 mailbox messages and 32 input events per
iteration before composition/presentation. Input remains ordered: buttons,
keys and capture changes are handled individually, with paint deferred to the
end of the batch. Old/new cursor rectangles stay separate rather than damaging
the full path bounding box. Client teardown returns bounded separate damage
regions for the released footprint, focus changes, taskbar and tiling.

Normalized HID tablet coordinates now route through the same pointer update,
hit testing, drag and capture path as PS/2 relative motion. The conversion is
in logical desktop pixels, including at scale 2, with clamped range endpoints.
The taskbar paints its own backdrop only, preserving desktop shortcuts.

The hardware-demo session explicitly requests the existing flat renderer and
reduced motion; other session defaults and the 3D settings toggle are kept.
Session argv uses seven bounded borrowed string slots and the existing ELF
loader copy; capabilities, syscall ABI and credential work factor are unchanged.
No new kernel objects, locks, unsafe code, asynchronous pointers or blocking
operations were introduced. RUN-DESKTOP.ps1 builds the demo or accepts an
explicit image, uses 5 GiB configured RAM, creates a USB tablet and records a
unique session directory with PID, local QMP port and serial/QEMU logs.
The shell release profile uses opt-level=2 rather than size optimization for the software-rendering and credential-KDF hot paths. LTO/stripping and the existing executable-size bound remain in force. The PBKDF2 algorithm, credential format, 100,000 iterations and kernel verification boundary are unchanged. Closing a window also clears its pointer capture so slot reuse cannot inherit a dead client's capture.
RUN-DESKTOP.ps1 defaults to the already-enabled Windows hypervisor accelerator (whpx), with explicit -Accelerator tcg for hosts without it. WHPX and tablet input were checked against the full demo image on this host; this launch configuration is separate from the unchanged TCG acceptance harnesses. No Windows features or host security settings were modified.

The interactive launcher snapshots the chosen image into its unique run directory
and passes it read-only to QEMU, preventing later Cargo builds from changing the
boot disk of newly launched VMs. Desktop composition remains software rendering;
WHPX accelerates guest CPU execution, not the graphics pipeline.


## Desktop graphics acceptance and repaint lifetime recovery - 2026-10-27

Flat functional contracts explicitly select --no-3d; dedicated 20.17/20.18
retain independent projected geometry and exact visible-toast checks. Stage
20.6 reads its header/body/shadow strip only between completed frames through
an explicitly feature-guarded compositor helper. It neither sleeps nor switches
tasks while the compositor lock is held.

Shell repaint may race client surface release after check_clients. A rejected
region blit now triggers a second liveness check. A vanished surface goes
through existing close teardown, clearing capture/drag and updating focus,
stack/workspace and tiling. Repaint restarts over the full screen before present
to repair all exposed state. At most MAX_WINDOWS removals and one final attempt
are possible; no new window is created in this single-threaded loop. A still-live
surface error remains logged and fatal. Existing resize replacement deadlines
remain intact. Kernel ownership/generation validation and all ABIs are unchanged;
there are no new locks, objects, unsafe blocks or asynchronous pointers.

Software acceptance and native/accelerated limitations are recorded in
[the acceptance audit](graphics/desktop-acceptance-2026-10-27.md).
