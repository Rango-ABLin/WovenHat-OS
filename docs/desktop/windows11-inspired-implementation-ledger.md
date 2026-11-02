# Native Windows-inspired WovenHat Desktop: implementation ledger

Branch: `feature/windows11-desktop-foundation-20261102`

## Status (2026-11-02)

**IN PROGRESS — NOT PRODUCTION READY.** This branch is the first bounded change, not completion of the eight-feature mandate.

The established `apps/woven-shell` Ring-3 session remains the only desktop owner.
No GNOME or Linux desktop service is introduced. No kernel syscall changes have
been made by this branch.

### Implemented in this branch

- 50 logical-pixel bottom taskbar geometry, replacing the previous 24-pixel value.
- 36-pixel Start control centred vertically, retaining the original WovenHat icon.
- 40-pixel running-application slots with 8-pixel horizontal spacing, centred
  within the remaining space subject to reserved Start and status regions.
- Focused-window accent indicator and icon-based taskbar entries.
- Bottom-right toast positioning above the taskbar.
- Pure, allocation-free helpers for work-area, snap and taskbar geometry.
- Host-oriented unit tests for geometry and small-display bounds.

### Already available in existing WovenShell (not new claims)

Current source includes a launcher, basic taskbar and clock, notifications,
window management and workspaces. Refer to `docs/graphics/current-state-audit.md`
and the accepted desktop evidence before changing their contracts.

### Not yet implemented/verified by this branch

- Super-key and Alt+Tab modifier routing, preview thumbnails, persistent pins,
  fullscreen launcher and live search improvements.
- Dedicated authenticated settings and action-center services; device-backed
  volume, battery, Wi-Fi and power controls; durable user preferences.
- Full keyboard snapping integration (the geometry helper alone is insufficient).
- 200ms transitions, actual performance qualification, 50 MiB footprint,
  startup response under 2 seconds and stability evidence.
- Complete screenshots, 1/2/4-CPU new-feature QEMU gates, native hardware,
  or a zero-warning Clippy result against this change.

## Mandatory verification before merge

1. Run shell freestanding target Clippy with warnings denied.
2. Run pure geometry host tests.
3. Run the existing system-apps sign-in/input/screenshot gates on 1, 2 and 4 CPUs.
4. Update screenshot references only after reviewing visual differences. Do not
   weaken an existing oracle to mask a regression.
5. Verify 640x400, 1280x800 and 1920x1080 layout; ensure status controls and
   taskbar icons do not overlap when many windows are open.
6. Test normal desktop, lock screen, launcher, notification center, keyboard,
   pointer, window create/close and compositor recovery.
7. Confirm the userspace shell's static scratch allocation remains within its
   memory budget after the taskbar height increase.

## Implementation sequence

- **Phase 1:** finish icon/status overflow, independent hit tests, clock,
  taskbar render gates and 1/2/4-CPU regression.
- **Phase 2:** real app metadata and pinned ordering, Super/Alt modifiers,
  fullscreen searchable launcher, MRU preview cache and focus routing.
- **Phase 3:** bounded/versioned IPC, capability-scoped settings daemon,
  notification history/action center, verified device status and authorized
  power actions, persisted theme and snapping.
- **Phase 4:** performance instrumentation and targets, stress/fault testing,
  screenshot comparison, production release review, physical hardware gates.

## Acceptance count

The request describes 18 criteria but enumerates **20** (10 functional,
4 performance, 6 quality). Track all 20 individually. None of the new
acceptance gates should be considered passed without execution evidence.

## Security

Keep display lease and input focus in WovenShell; do not grant arbitrary
clients framebuffer ownership, screenshots of protected windows, global
shortcut interception, privileged power requests or cross-session access.
