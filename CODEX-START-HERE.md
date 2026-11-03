# CODEX: WINDOWS 11 DESKTOP IMPLEMENTATION

## Your Assignment

Build a Windows 11-inspired desktop environment for WovenHat-OS with a **bottom taskbar** (not GNOME top bar).

**Status:** ✅ SPECIFICATION COMPLETE - READY TO START IMMEDIATELY

---

## Quick Start

1. **Read this first:**
   - `docs/CODEX-Windows11-Assignment.txt` (executive brief, 2 min read)

2. **Technical specification:**
   - `docs/Windows11-Desktop-Specification.md` (full spec, 10 min read)

3. **Verification criteria:**
   - `docs/Implementation-Verification-Checklist.json` (acceptance gates)

4. **Then start Phase 1:**
   - Create project structure for `wovenhat-windows-shell`
   - Implement taskbar at bottom (50px height)
   - Show Start button and clock
   - Report when visible

---

## The 8 Core Features You're Building

1. **Bottom Taskbar** (50px)
   - Windows logo/Start button (left)
   - App icons: pinned + running (center)
   - Clock, Battery, Volume, WiFi, Settings (right)

2. **Start Menu** (Super key)
   - App launcher with search
   - Recently used apps
   - All apps grid (categories)
   - Pinned apps management

3. **App Switcher** (Alt+Tab)
   - Shows running apps with thumbnails
   - Hover over taskbar to see all windows

4. **System Tray**
   - Clock/Date, Battery %, Volume slider
   - WiFi status, Settings, Power menu

5. **Notifications**
   - Toast popups (bottom-right)
   - Action Center (notification history)
   - Do Not Disturb mode

6. **Window Management**
   - Snap: Super+Left/Right (half-screen)
   - Maximize: Super+Up
   - Minimize: Super+Down

7. **Settings Integration**
   - Display, Sound, Network, Power
   - Persistent config

8. **Theme System**
   - Light/dark modes
   - Accent color customization

---

## Timeline: 8 Days, 4 Phases

| Phase | Days | Focus | Deliverable |
|-------|------|-------|-------------|
| 1 | 1-2 | Foundation | Taskbar visible with time |
| 2 | 3-4 | Core Features | Start Menu, app icons, Alt+Tab |
| 3 | 5-6 | Integration | Tray, notifications, snap, theme |
| 4 | 7-8 | Quality | Performance, polish, QEMU gates |

---

## Success Criteria (ALL Must Pass)

**Functional (8):**
- ✓ Taskbar renders at bottom
- ✓ Start Menu opens (Super key)
- ✓ App icons on taskbar
- ✓ Alt+Tab with previews works
- ✓ Hover preview shows windows
- ✓ System tray functional
- ✓ Notifications appear/dismiss
- ✓ Snap layouts work

**Performance (4):**
- ✓ Taskbar: 60 FPS (16ms per frame)
- ✓ Memory: < 50MB total
- ✓ Startup: < 2 seconds
- ✓ Search: < 100ms response

**Quality (6):**
- ✓ Clippy: ZERO warnings
- ✓ QEMU: 1 CPU gate pass
- ✓ QEMU: 2 CPU gate pass
- ✓ QEMU: 4 CPU gate pass
- ✓ No crashes/hangs
- ✓ Visual polish complete

---

## Deliverables

**3 Applications:**

1. `wovenhat-windows-shell` (main, 8-10 modules)
   - taskbar.rs, start_menu.rs, app_switcher.rs
   - system_tray.rs, notifications.rs, window_manager.rs
   - keybindings.rs, theme.rs, ipc.rs

2. `wovenhat-settings-daemon` (background service)
   - Display, audio, network, power settings

3. `wovenhat-action-center` (notification hub)
   - Notification history, Do Not Disturb

---

## Review Checkpoints

**Claude Code will verify:**

After Phase 1:
- ✓ Taskbar visible at bottom
- ✓ Clock displays correctly
- ✓ Clippy passes (zero warnings)
- ✓ WovenShell integration works

After Phase 2:
- ✓ Start Menu launches
- ✓ App icons appear on taskbar
- ✓ Alt+Tab shows windows
- ✓ Hover previews work

After Phase 3:
- ✓ All 8 features functional
- ✓ Performance metrics collected
- ✓ No regressions

After Phase 4:
- ✓ All 18 acceptance criteria pass
- ✓ Performance targets met
- ✓ QEMU gates pass (1/2/4 CPU)
- ✓ Final go/no-go decision

---

## Performance Targets (Non-Negotiable)

```
Taskbar rendering:        60 FPS (16ms per frame)
Start Menu animation:     smooth 0.2s transition
App Switcher response:    instant (< 50ms)
Memory footprint:         < 50MB total
Startup time:             < 2 seconds
Search response:          < 100ms
```

---

## Code Quality Requirements

- Language: Rust (no unsafe unless graphically necessary)
- Clippy: `cargo clippy -- -D warnings` (ZERO warnings)
- Target: x86_64-unknown-none (no_std kernel)
- Dependencies: Only WovenHat ecosystem
- No new kernel changes required

---

## Resources in This Repo

**Study these first:**
- `apps/woven-shell/` (window manager integration)
- `kernel/src/display.rs` (graphics subsystem)
- `libwoven/src/graphics/` (rendering library)
- `scripts/test-desktop-qualification.py` (QEMU acceptance gate)

**Existing syscalls to use:**
- Graphics: `sys.display.blit`, `sys.display.screenshot`
- Windows: `sys.window.create`, `sys.window.focus`, `sys.window.raise`
- Input: keyboard/mouse event delivery
- Storage: filesystem for apps and settings

---

## Communication

**Daily Status:**
- Brief note on completed modules
- Any blockers or questions

**After Each Phase:**
- Full demo (screenshot or video)
- Performance data if collected
- Status: Ready for next phase or needs fixes

---

## Go/No-Go Decision

**READY FOR PRODUCTION if:**
- ✓ All 18 acceptance criteria verified
- ✓ Performance targets confirmed
- ✓ Clippy: zero warnings
- ✓ QEMU: 1/2/4 CPU gates pass
- ✓ No crashes or regressions
- ✓ Visual polish complete

**NOT READY if:**
- ✗ Any acceptance criterion fails
- ✗ Performance below targets
- ✗ Clippy warnings present
- ✗ QEMU gate fails on any CPU count
- ✗ Regressions in existing features

---

## Start Now

1. **Read:** `docs/CODEX-Windows11-Assignment.txt` (2 min)
2. **Review:** `docs/Windows11-Desktop-Specification.md` (10 min)
3. **Begin:** Phase 1 implementation
4. **Report:** Status when taskbar is visible

---

**Assignment Status:** ✅ ACTIVE - CODEX BEGINS IMMEDIATELY

Claude Code is standing by for Phase 1 review.

Good luck! Users are waiting for this desktop. 🚀
