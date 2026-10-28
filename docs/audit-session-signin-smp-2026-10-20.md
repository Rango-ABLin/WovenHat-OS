# WovenShell sign-in/session SMP audit — 2026-10-20

## Change

WovenShell lock-screen input now returns bounded damage. Account selection
repaints the account rows and PIN prompt; digit, backspace, and wrong-PIN
states repaint only the prompt/dot region; successful sign-in requests the
full desktop repaint. This removes unnecessary full-screen wallpaper work on
each key and fixes the multicore visual timing failure without weakening the
assertion.

## Acceptance

Stage 20.12 sign-in passed on QEMU 1/2/4 CPUs:

- sign-in screen and initial account selection;
- Tab account selection;
- wrong-PIN refusal;
- successful `lin` sign-in and taskbar identity;
- lock-screen wrong-PIN refusal and unlock;
- return of display ownership to the terminal.

Evidence is retained in `target/woven-shell-sign-in-{1,2,4}cpu`.

Stage 20.15 session passed on QEMU 1/2/4 CPUs:

- session display/input focus;
- launcher listing;
- F12 console handoff and typed console key;
- return to desktop;
- Quick Settings and guest power-off.

Evidence:

- `audit-artifacts/stage20.15-1cpu-1792475698900665700`
- `audit-artifacts/stage20.15-2cpu-1792476022630351800`
- `audit-artifacts/stage20.15-4cpu-1792476101877791200`

## Remaining boundary

The interactive `hardware-demo` path still creates the `ada/1234` and
`lin/9876` sample accounts in volatile VFS state. Persistent account
installation and release-image boot remain the next session slice.
