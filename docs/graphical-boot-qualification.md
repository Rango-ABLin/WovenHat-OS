# Automatic graphical boot — production acceptance plan

Status: **OPEN / NOT IMPLEMENTED**. Source inspection on master `68751dd427c56c8a78d8646b73e5ec9a893a138c` confirms that `kernel/src/main.rs` uses `let mut desktop_active = false;` and explicitly starts the diagnostic shell; F1 toggles the kernel GUI. This is a real boot-experience gap, not a CI failure.

## Target
Boot normally to a graphical login/session and desktop without a keypress, with a consistent Windows 11–inspired experience (original WovenHat branding). Do not imitate proprietary code or assets. Keep diagnostic shell reachable through an explicit recovery path, not the default boot screen.

## Required architecture
1. Separate early serial/framebuffer diagnostics from the normal session manager. Initialize graphics, input and compositor before launching a graphical login/session; report visible progress and explicit failures.
2. Avoid racing kernel keyboard polling against userspace foreground ownership; make input focus explicit. Preserve serial/headless and safe-mode recovery paths.
3. Define a single desktop authority: the long-term Ring-3 WovenShell/compositor rather than the current kernel-only demo window. Keep startup failures observable and recoverable.
4. Qualify real storage/root discovery, persistence, filesystem failure behavior, GPU/framebuffer and display modes, HID/USB, networking and suspend/resume only when implemented and supported.
5. Provide graphical login with rate-limited secure authentication after issue #72; no silent bypass or auto-login to a privileged session.

## Mandatory acceptance
- Normal QEMU UEFI boot at 1, 2 and 4 CPUs must show graphical session automatically, without F1; verify framebuffer evidence/screenshots or video, serial boot milestones, user input and responsiveness.
- Test missing/corrupt root volume, missing framebuffer, compositor crash/restart, invalid credentials, keyboard focus, concurrent window activity and fallback recovery shell.
- Run warnings-denied Clippy, unit and integration tests, 1/2/4 CPU acceptance, filesystem reboot/remount and driver regressions; archive evidence at exact SHA.
- Verify supported physical hardware, power-loss behavior, installer/recovery, updates/rollback and security before calling this production-ready.

## Current blockers
Stage 1–19 closure ledger is still BLOCKED; issue #72 legacy SHA-256 PIN verifier; physical hardware and installer/recovery evidence missing. No source boot change is claimed by this plan.
