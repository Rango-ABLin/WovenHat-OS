# WovenFS-as-system-filesystem migration — tracker and resume map

**Status: PARTIAL, NOT ACCEPTED (2026-11-04).** Phase 0 audit is recorded
and the first production-routing/provisioning increment is implemented and
verified on disposable media; installed-image boot qualification and the full
storage-neutral service audit remain open.
The user explicitly authorized switching priority from the still-open graphics
modernization work to this migration. The graphics sequencing sentence below
is superseded for this task only; the graphics claim and files remain intact.

## The decision (resolves a previously open product decision)

`docs/stage-status.md` (Stage 15.2 section) has long recorded: "Open:
WovenFS is not the mounted system filesystem -- FAT32 still holds `/mnt`
and the boot volume -- and whether to make it one or to keep FAT32 and
reclassify Stage 12.2/12.4 is an open product decision."

The user resolved this on 2026-11-03, after the pointer-jitter/graphics-
sharpness follow-up was requested:

> "Make WovenFS the system filesystem, and keep FAT32 only for the EFI
> boot partition and removable media. Don't reclassify 12.2 and 12.4."

So, concretely:

- **WovenFS becomes the system filesystem** — it replaces FAT32 as what
  `/mnt` (or wherever the system root mounts) is backed by, for the
  non-removable, non-EFI storage WovenHat owns.
- **FAT32 stays, scoped down to exactly two roles**: the EFI System
  Partition (boot firmware requires FAT32; this is not negotiable, UEFI
  firmware reads it directly) and removable media (USB sticks etc., for
  interoperability with other OSes reading/writing them).
- **Stage 12.2 and Stage 12.4 keep their existing ACCEPTED status and
  scope exactly as recorded in `docs/stage-status.md` — do not touch,
  rewrite, re-date, or reclassify those two stage-status entries as part
  of this migration.** This was explicit and deliberate in the user's
  instruction ("Don't reclassify 12.2 and 12.4"), presumably because
  resolving the "or keep FAT32 and reclassify 12.2/12.4" branch of the
  old open-decision note is exactly the path *not* taken — the other
  branch (make WovenFS the system FS) was chosen instead, so 12.2/12.4
  need no reclassification; they simply remain accepted as they already
  are. If a future session is tempted to "clean up" or re-word those two
  stage-status sections while doing this migration, don't — that's the
  one thing explicitly ruled out.

## Why this is a big task — read before starting

This is not a config flag flip. Making WovenFS the real system filesystem
touches:

- The boot/install path: whatever currently formats/provisions the system
  volume as FAT32 (check `kernel/src/main.rs`'s boot sequence, the
  storage manager from Stage 12.5, and `apps/woven-installer`) needs to
  provision WovenFS instead, keeping FAT32 only for the ESP it creates
  alongside it.
- `/mnt`'s mount path and every piece of code that assumes FAT32
  semantics there (directory entry format, name-length limits, case
  sensitivity, free-space reporting) — audit before assuming WovenFS's
  existing `/wfs` mount can just be remapped to `/mnt` unchanged.
  Stage 15.2's own notes describe WovenFS's current native-path/VFS
  adapter as still maturing (serialized read-modify-write transactions,
  "mounted-system publication... walks validated nested directories",
  "Mounted-system policy remains open" as of that stage's writing) --
  re-read that section of `docs/stage-status.md` in full before starting,
  since it already anticipates exactly this transition and may have
  since moved further.
- Every app/service that currently assumes it's reading/writing FAT32 at
  `/mnt` — survey for FAT32-specific assumptions (8.3 names, cluster
  semantics, etc.) the same way the app visual-token overhaul surveyed
  every app's color constants, before assuming a drop-in swap is safe.
- Live-USB/removable-media handling must keep working on FAT32
  unchanged — this migration must not regress Stage 12.5's storage
  manager or removable-device mounting at all; it only changes what the
  *non-removable system volume* is formatted as.
- Crash-consistency and physical-hardware qualification: Stage 15.2's own
  notes say "Crash recovery is proven by fault injection, not actual
  power loss on a device" and physical qualification of destructive
  WovenFS operations was still incomplete as of that writing (needs
  Administrator privileges for the harness). Making WovenFS the system
  filesystem raises the stakes of any unproven crash-consistency gap
  enormously (it's now where user data and the OS itself live, not an
  optional `/wfs` mount) — do not skip re-verifying this just because
  the feature functions.

## Before starting: mandatory first steps

1. Re-read this file, `docs/graphics/modernization-tracker.md` (confirm
   it's actually done or the user redirected focus), `AGENTS.md`,
   `docs/stage-status.md`'s Stage 15/15.2/12.2/12.4/12.5 sections in full,
   `docs/agent-coordination.md`.
2. Claim this work in `docs/agent-coordination.md` the same way the
   graphics mandate and shell takeover were claimed, before editing
   anything.
3. Do the equivalent of a Phase 0 audit for this task specifically: find
   every place FAT32 is assumed for the system volume (boot/install code,
   storage manager defaults, any hardcoded "FAT32" string or format
   choice for the non-removable disk), and every place WovenFS's
   `/wfs`-mount behavior differs from what `/mnt`'s current consumers
   expect. Write the defect/gap inventory down (in this file, as a new
   section) before changing code, the same discipline used for the
   graphics mandate.
4. This is boot-path and storage-format work — treat it with at least as
   much care as the AGENTS.md rules demand for new kernel objects: owner
   teardown, crash-consistency, cross-CPU races, bounded capacity, lock
   order. A corrupted or unbootable system volume is a much worse failure
   mode than a cosmetic graphics bug.

## Phase 0 defect and gap inventory — 2026-11-04

This inventory is based on the current clean tree and source trace. It is an
audit record, not an acceptance claim.

### Current ownership and mount paths

- `kernel/src/wfs.rs` is the native on-disk WovenFS implementation. Its real
  device adapter is currently `stage15-2-test`-gated, uses the primary ATA
  device directly, imports a bounded read-only namespace under `/wfs`, and
  deliberately does not hook generic VFS descriptor/syscall dispatch into its
  imported nodes. Its mount lifecycle invalidates `/wfs` children, replays and
  checks the journal, and fails closed on corruption.
- `kernel/src/wovenfs.rs` is not the native filesystem. It is a fixed 64-entry
  RAM-only metadata/xattr side table; it has no on-disk format, inode allocator,
  directory tree, data blocks, or reboot persistence. It must remain distinct.
- `kernel/src/storage.rs::mount_ata_root` is the production `/mnt` mount.
  It labels the root by probing FAT32, mounts a direct FAT32 volume first,
  then an MBR FAT32 partition, then a GPT FAT partition. It records
  `MountStatus::NotFat32` and imports directory entries into the VFS.
- The same storage module implements `/mnt` read-through page caching,
  `persist_path`/bounded batch persistence, WDJ1/WDJ2 rollback journals,
  `df_mnt`, `fscheck_mnt`, remount, and teardown against FAT32. These are not
  drop-in WovenFS calls: they depend on FAT32 names, metadata sidecars,
  directory creation and path resolution.
- `kernel/src/storage_manager.rs` Stage 12.5 discovery only detects FAT32
  partitions and publishes `FilesystemKind::Fat32`; its accepted removable
  metadata/mount/teardown behavior must remain available.
- `kernel/src/main.rs` invokes `storage::mount_ata_root` during boot and later
  service initialization, reports FAT32-specific status text, and separately
  exposes Stage 15.2's WovenFS test path. No production boot path selects
  WovenFS for the non-removable system volume.

### Provisioning and role confusion

- `scripts/validate-installer-plan.py` accepts `ext4`, `fat32`, and `wovenfs`
  as plan values, but `scripts/execute-disk-install.py` accepts only `fat32`;
  its formatter and verifier operate on the selected image as one FAT32
  volume. This is a schema/executor mismatch, not support for WovenFS.
- Installed-image and live-USB/QEMU fixtures intentionally create GPT images
  with a FAT32 ESP and/or FAT32 data/log media. The ESP must remain FAT32 for
  UEFI, removable media must remain FAT32 for interoperability, and only a
  non-removable system partition may become WovenFS.
- Current root discovery does not identify the ESP versus the system
  partition, does not require an ownership marker before destructive format,
  and has no WovenFS partition detector in the storage manager. A naive
  “try WovenFS first” change could mount or format the wrong partition and
  break foreign/removable FAT32 behavior.

### Consumer and semantic mismatches

- `/mnt` consumers use generic VFS paths, while WovenFS's adapter is
  `/wfs`-specific and only its explicit authorized adapters are writable.
  `kernel/src/task.rs` has separate `/mnt` FAT32 persistence and `/wfs`
  native-authority branches; these must be reconciled without widening
  userspace authority or changing the syscall ABI.
- `/mnt` FAT32 import lowercases short names, accepts FAT long-name rules,
  bounds files at `vfs::NODE_CAPACITY`, and overlays metadata recovered from
  FAT journals. WovenFS has its own component/name, inode, directory, xattr,
  file-size, and case-sensitive rules. Migration needs explicit compatibility
  tests for names, directories, metadata, size/free space, rename/unlink, and
  persistence.
- Kernel services persist durable update/TPM records under `/wfs`, while other
  services and fixtures use `/mnt`. The migration must decide whether these
  paths alias one system volume or remain separate namespaces, then preserve
  service recovery and teardown semantics.
- Some userspace/service fixtures gate `/mnt` writes on
  `storage::fat32_writable`; this filesystem-specific check must become a
  storage-neutral system-volume check only after equivalent WovenFS write-
  through and recovery authority exists.

### Safety and qualification gaps

- `tests/wovenfs_crash.rs` proves bounded journal crash recovery on disposable
  images, including refusal beyond the current direct-block limit. The
  rejected unguarded block-tree attempt must not return; no capacity or format
  expansion is authorized by this migration.
- Stage 15.2 evidence covers WovenFS format/mount and bounded VFS import on
  scratch/emulated and previously qualified removable media. It does not
  prove a bootable installed WovenFS system volume, arbitrary power-loss
  recovery, or a native system-volume migration.
- Physical qualification scripts are destructive and require explicit device
  authorization/elevation. This task authorizes no physical-device writes; all
  implementation tests use disposable image files.
- Mount teardown, open-description invalidation, page-cache invalidation,
  journal replay, cross-CPU serialization, and ownership of partition/device
  views need dedicated review before generic `/mnt` access is redirected.

## Bounded implementation plan

1. Add storage-neutral role/probe primitives that distinguish the
   non-removable system partition from the FAT32 ESP and removable media,
   with fail-closed detection and no destructive fallback. Keep Stage 12.5's
   FAT32 removable path unchanged and add disposable-image probe tests.
2. Generalize the native WovenFS mount/import lifecycle from the test-only,
   primary-ATA `/wfs` adapter to an explicitly selected system-volume device
   view, retaining bounded traversal, checksum/consistency checks, serialized
   mutations, cache invalidation, and stale-handle teardown. Do not expose
   generic writes until the authority boundary is reviewed.
3. Introduce the smallest storage-neutral `/mnt` backend boundary and route
   read/stat/dir/metadata/space/check operations through WovenFS for the system
   role. Preserve `/wfs` compatibility as an intentional alias or separate
   service namespace, documented and tested.
4. Port durable system-volume persistence, update/TPM/recovery paths, and
   installer/provisioning to WovenFS using the existing bounded journal/format;
   keep FAT32 format/mount/persistence for ESP/removable roles. Any installer
   format change remains disposable-image-only until verifier and rollback
   evidence pass.
5. Run focused host tests, `tests/wovenfs_crash.rs`, warning-denying host and
   target Clippy for every affected feature, and relevant QEMU chains at
   1/2/4 CPUs. Only after those pass, run prior-stage storage gates and
   installed-image boot qualification. Physical power-loss/removable hardware
   limits remain explicit.

The first implementation increment is intentionally limited to role-aware
selection and a mount-path boundary. It must not silently format, fall back
to an empty WovenFS, alter accepted Stage 12.2/12.4 text, or claim full
migration.

## Resume log — 2026-11-04 — routing/provisioning increment

Implemented and verified in the working tree:

- `mount_ata_root` probes WovenFS before FAT32 and probes validated GPT
  EFI/basic-data entries for a WovenFS system partition. FAT32 fallback is
  unchanged for ESP/removable paths; no physical device was opened.
- `/mnt` can publish a selected WovenFS volume. Task writes, mkdir, unlink,
  rename, ensure/read, unmount, and stale VFS child invalidation route through
  the native transaction path. The lock-order panic found by QEMU was fixed
  and re-run successfully.
- The disposable installer accepts `filesystem: wovenfs`, formats the data
  region with the exact WovenFS v1 superblock/bitmap/inode/root layout,
  embeds settings, verifies determinism, and preserves rollback behavior.

Evidence completed: installer WovenFS round-trip tests; `cargo test --test
wovenfs_crash` 20/20; workspace tests; warning-denying target and host
Clippy; Stage 12.1–12.5 QEMU at 1/2/4 CPUs; WovenFS device harness at 1/2/4
CPUs. Evidence paths are the fresh `audit-artifacts/stage12.*` directories and
`target/wovenfs-device-scratch-{1,2,4}cpu-*` directories from 2026-11-04.

Still not accepted: no physical writes were authorized, no destructive
physical qualification was run, no installed WovenFS system image was booted
through the full installer-to-boot path, and FAT32-specific `df`/`fscheck`
and all service persistence callers have not yet been fully made backend
neutral. The migration remains partial until those gates are closed.

## Resume log (append a dated entry every time you stop)

### 2026-11-03

Task queued by the user, sequenced after the graphics modernization
mandate and its pointer-jitter/sharpness follow-up. This tracker file
created so the decision and its scope survive future credit resets. No
audit or implementation work done yet.

### 2026-11-04

The user explicitly switched priority to this migration. Read the required
repository instructions, stage/architecture/coordination documents, Stage
12.2/12.4/12.5 and Stage 15/15.2 records, and the graphics tracker. Working
tree was clean. Claimed the filesystem/storage paths in
`docs/agent-coordination.md` and recorded the Phase 0 defect inventory and
bounded plan above before source implementation. No physical device has been
opened or modified.

The first probe increment was then implemented and verified. It adds
`wfs::Volume::probe` and the Stage 12.5-only
`storage_manager::inspect_system_device` API; existing FAT32 discovery and
all accepted filesystem behavior remain unchanged. Host/target lint, the
20-test crash suite, and the Stage 12.1–12.5 QEMU chain passed. The migration
remains partial and not accepted.
