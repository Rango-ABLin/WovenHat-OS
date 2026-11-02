# WovenHat-OS GNOME-Inspired Desktop Implementation

## Executive Summary

Implement a modern desktop environment for WovenHat-OS inspired by GNOME's user experience, integrated with the existing WovenShell compositor and kernel graphics subsystem.

## Core Features

1. **Activities Overview** - App launcher + window switcher (Super key to activate)
2. **Top Bar** - 40px system panel with clock, status icons, power menu
3. **Notifications** - Toast popups and notification center
4. **App Launcher** - 4x3 grid with search and categories
5. **Window Management** - Super+arrows for snap, Alt+Tab switcher
6. **Settings Integration** - Display, Sound, Network, Power, Appearance
7. **Keyboard Shortcuts** - Full keyboard navigation support
8. **Theme System** - Light/dark mode with accent colors

## Technical Architecture

wovenhat-gnome-shell (main shell service)
  - top_bar.rs: system panel rendering
  - activities.rs: overview and launcher
  - notifications.rs: notification daemon
  - window_manager.rs: window integration
  - keybindings.rs: keyboard shortcuts
  - theme.rs: styling engine
  - ipc.rs: message passing

wovenhat-settings-daemon (background service)
  - display.rs: screen settings
  - sound.rs: audio control
  - persistence.rs: config storage

## Implementation Timeline

Phase 1: Foundation (Days 1-2)
  - Project setup, IPC framework, top bar

Phase 2: Core (Days 3-4)
  - Activities, launcher, window switcher, workspaces

Phase 3: Polish (Days 5-6)
  - Notifications, settings, theme, shortcuts

Phase 4: Testing (Days 7-8)
  - Performance profiling, QEMU gates, bug fixes

## Performance Targets

- Top bar: 60 FPS (16ms per frame)
- Activities: smooth 0.3s animations
- App search: < 100ms response
- Memory: < 50MB total
- Startup: < 2s to responsive desktop

## Quality Criteria

Success means:
- Desktop boots to Activities-ready state
- All 8 core features functional
- 60 FPS performance maintained
- Clippy: zero warnings
- QEMU: all gates pass (1/2/4 CPU)
- Memory footprint < 50MB
- No regressions in existing features

## Implementation Order (Recommended)

1. wovenhat-gnome-shell foundation + IPC
2. Top bar (visible, simplest)
3. Activities overview (core feature)
4. App launcher + search
5. Notifications system
6. Keyboard shortcuts + window management
7. Settings integration
8. Theme system + final polish

## Code Quality Requirements

- Clippy: zero warnings
- All modules warning-denying
- Host tests must pass
- QEMU qualification at 1/2/4 CPUs
- Memory profiling < 50MB
- Visual regression tests against spec
