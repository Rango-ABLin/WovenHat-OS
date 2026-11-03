# WovenFS-as-system-filesystem migration — tracker and resume map

**Status: QUEUED, not started.** Do not begin this work until
`docs/graphics/modernization-tracker.md`'s Current status table shows the
graphics modernization mandate complete (or the user explicitly says to
switch focus). This file exists now, ahead of time, purely so the decision
and scope are captured durably and don't get lost or re-litigated later.

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

## Resume log (append a dated entry every time you stop)

### 2026-11-03

Task queued by the user, sequenced after the graphics modernization
mandate and its pointer-jitter/sharpness follow-up. This tracker file
created so the decision and its scope survive future credit resets. No
audit or implementation work done yet.
