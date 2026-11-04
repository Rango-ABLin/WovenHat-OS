# WovenHat-OS Windows 11-Style Desktop Implementation

## Key Change: Bottom Taskbar (NOT Top Bar)

This specification describes a Windows 11-inspired desktop with:
- Bottom taskbar (50px height)
- Centered app icons
- Start menu launcher
- System tray (right side)
- NO top bar or Activities overlay

## Core Features (8 Total)

1. Bottom Taskbar
   - Windows logo/Start button (left)
   - Pinned & running app icons (center)
   - System clock (right)
   - System tray icons (far right)

2. Start Menu / App Launcher
   - Fullscreen or side panel overlay
   - App grid with categories
   - Search functionality
   - Recently used apps
   - Pinned apps management

3. App Switcher
   - Alt+Tab shows running apps with thumbnails
   - Hover over taskbar icon shows window previews
   - Click to switch or launch

4. System Tray
   - WiFi/Network status
   - Volume control slider
   - Battery indicator with percentage
   - Current time/date
   - Settings and Power menu

5. Notifications System
   - Toast popups (bottom-right, above taskbar)
   - Auto-dismiss in 4 seconds
   - Action Center with notification history
   - Do Not Disturb toggle

6. Window Management
   - Snap layouts: Super+Left/Right (half-screen)
   - Super+Up: Maximize
   - Super+Down: Restore/Minimize
   - Virtual desktops/workspaces

7. Settings Integration
   - Display, Sound, Network, Power, Appearance
   - Accessible from Settings app

8. Theme System
   - Light and dark modes
   - Accent color customization
   - Semi-transparent/mica effects

## Architecture

Three applications:

1. wovenhat-windows-shell (main)
   - taskbar.rs: 50px bottom panel
   - start_menu.rs: app launcher
   - app_switcher.rs: Alt+Tab + previews
   - system_tray.rs: clock, battery, volume, icons
   - notifications.rs: toast system
   - window_manager.rs: snap, focus, minimize
   - theme.rs: light/dark/accent colors

2. wovenhat-settings-daemon (background service)
   - Display, Audio, Network, Power settings

3. wovenhat-action-center (notification hub)
   - Notification history and management

## Timeline: 8 Days (4 Phases)

Phase 1 (Days 1-2): Foundation
- Taskbar rendering at bottom
- Start button and clock
- WovenShell integration

Phase 2 (Days 3-4): Core Features
- Start Menu launcher
- App icons on taskbar
- Alt+Tab switcher with previews
- Window preview on hover

Phase 3 (Days 5-6): System Integration
- System tray (volume, battery, network, settings)
- Notifications (toasts + action center)
- Window snap layouts
- Theme switching

Phase 4 (Days 7-8): Quality & Performance
- Visual polish (animations, spacing, shadows)
- Performance optimization (60 FPS)
- Memory optimization (< 50MB)
- QEMU acceptance (1/2/4 CPU)

## Performance Targets

- Taskbar: 60 FPS (16ms per frame)
- Start Menu animation: smooth 0.2s
- App Switcher: instant (< 50ms)
- Memory: < 50MB total
- Startup: < 2 seconds

## Quality Requirements

- Clippy: ZERO warnings
- QEMU: all gates pass (1/2/4 CPU)
- No crashes in extended usage
- Visual regression tests pass
- Settings persistence verified

## Success Criteria

ALL of these must pass:

Functional:
✓ Taskbar renders at boot
✓ Start Menu opens with Super key
✓ App icons show on taskbar
✓ Alt+Tab works with previews
✓ Taskbar hover shows windows
✓ System tray shows time/battery/volume
✓ Notifications appear and auto-dismiss
✓ Window snap layouts work
✓ Themes switch (light/dark)
✓ Settings persist after reboot

Performance:
✓ 60 FPS verified
✓ < 50MB memory
✓ All animations smooth

Quality:
✓ Clippy: zero warnings
✓ QEMU: 1/2/4 CPU gates pass
✓ No regressions
✓ Visual polish complete
