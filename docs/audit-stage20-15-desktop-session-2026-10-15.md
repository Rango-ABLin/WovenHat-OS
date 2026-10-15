# Stage 20.15: the WovenHat desktop as the session audit — 2026-10-15

## The gap

A normal or release boot never started the desktop. WovenShell ran only
inside the Stage 20 test gates, so a real boot showed the kernel's text
console and its early demo window.

## What changed

**Session (`kernel/src/session.rs`).**
- A system-apps boot starts WovenShell as the session, with sign-in when
  `/etc/wovenhat/accounts` exists.
- The session holds Console, DisplayIo, InputIo, Ipc, FileRead,
  ProcessCreate, AudioPlayback and PowerControl.
- Its launcher lists the installed system applications.

**F12, the diagnostic console.**
- PS/2 scancode 0x58 or USB usage 0x45 switches between the desktop and
  the kernel's console.
- The key is the kernel's own. WovenInput intercepts it before routing, so
  it never reaches the userspace ABI.
- In console mode the compositor holds back the session's frames, and the
  terminal gets the display and the keyboard.
- Returning repaints the whole surface and gives focus back.
- Testers keep `note`, `savelog` and `poweroff`.

**Power control.**
- Capability `PowerControl` (25) is within the system-service ceiling
  only; applications never get it.
- Syscall 209, `PowerControl`: 0 powers off, 1 restarts. It is audited as
  `Action::PowerControl`.
- `libwoven::power_off` and `restart` wrap it.
- Quick Settings gains "Power off" and "Restart" at the top. The panel
  grows upward, so every existing row keeps its screen position.
- A refusal raises a toast with the reason.

**Release flow.**
- `test-release.py` and `test-shell-qemu.py` build WovenShell and embed it
  (`WOVENHAT_SHELL_IMAGE`).
- The system-apps boot check opens the console with F12 before typing
  `smptest`.
- WovenShell's sources are in the validated-image hash.

## Bugs found by this stage

- **ET_EXEC images were relocated under ASLR (Stage 11 loader).**
  - Production kernels randomise every image's bias, but an ET_EXEC image
    holds absolute addresses.
  - WovenShell, linked at 0x400000000000, faulted on an instruction fetch
    at 0x4000000005c1 the first time a production kernel loaded it.
  - Test kernels use a zero bias, so no gate had seen it.
  - `elf::Image::is_position_independent` now records the image type, and
    `load_elf` randomises only ET_DYN images.
- **The shell image outgrew the VFS's 64 KiB file limit** (66,928 bytes).
  The Stage 20.1–20.3 gates, which install it into the VFS, failed. The
  release profile now strips symbols (60,904 bytes).

## Gate

`scripts/test-stage20-15-session.py`. The session starts through
`session::start`, with an application in the catalog. With real i8042
input:
1. the launcher lists the application;
2. F12 shows the console: the session's frames stop, the terminal owns
   the display, and a typed key reaches the kernel shell;
3. F12 returns, restoring display and focus;
4. Quick Settings' Power off turns the machine off, and QEMU must exit by
   itself after the click.

The release-style system-apps boot also starts the desktop, opens the
console with F12, and runs `smptest` there.

## Evidence (`audit-artifacts/`)

- **20.15, 1/2/4 CPUs:** `stage20.15-1cpu-1792080508992804300`, `stage20.15-2cpu-1792077643020280800`, `stage20.15-4cpu-1792077659800304500`.
- **Release gate:** `release-validation-1792079997/`. It ran after both
  fixes; its validated image is the one written to the live-test stick.

Chain results:

```
clippy woven-shell: ok
clippy hello-gui: ok
lint-all-features: exit 0 []
host tests: ok
libwoven tests: ok
20.15 1cpu: PASS (35s) 
20.15 2cpu: PASS (15s) 
20.15 4cpu: PASS (20s) 
negative control 20.15 F12 not intercepted: caught ['QEMU exit status 35, expected 0 (turned off by the guest)', "[S20.15] F12 shows the diagnostic console; the session's frames stop FAIL", '[S20.15] desktop session: FAILED', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage20.15-1cpu-1792077682633163100']
negative control 20.15 console mode does not hold the session back: caught ['QEMU exit status 35, expected 0 (turned off by the guest)', '[S20.15] a key typed at the console reaches the kernel shell FAIL', '[S20.15] desktop session: FAILED', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage20.15-1cpu-1792077733739535800']
negative control 20.15 session without PowerControl: caught ['QEMU exit status 35, expected 0 (turned off by the guest)', '[S20.15] the machine turned off from the desktop FAIL', '[S20.15] desktop session: FAILED', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage20.15-1cpu-1792077770727566600']
20.1 1cpu: FAIL (28s) ['[S20.1] install into the filesystem FAIL', '[S20.1] woven-shell desktop: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-1cpu', '[S20.1] install into the filesystem FAIL']
20.2 1cpu: FAIL (29s) ['[S20.2] install shell and application FAIL', '[S20.2] desktop applications: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-apps-1cpu', '[S20.2] install shell and application FAIL']
20.3 1cpu: FAIL (27s) ['[S20.3] catalog and programs installed FAIL', '[S20.3] application launcher: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-launcher-1cpu', '[S20.3] catalog and programs installed FAIL']
20.4 1cpu: PASS (33s) 
20.5 1cpu: PASS (34s) 
20.6 1cpu: PASS (34s) 
20.7 1cpu: PASS (34s) 
20.8 1cpu: PASS (38s) 
20.9 1cpu: PASS (27s) 
20.10 1cpu: FAIL (40s) ['[S20.10] the Quick Settings button opens its panel FAIL', '[S20.10] quick settings: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-quick-settings-1cpu', '[S20.10] the Quick Settings button opens its panel FAIL']
20.11 1cpu: PASS (57s) 
20.12 1cpu: PASS (36s) 
20.13 1cpu: PASS (34s) 
20.3 2cpu: FAIL (13s) ['[S20.3] catalog and programs installed FAIL', '[S20.3] application launcher: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-launcher-2cpu', '[S20.3] catalog and programs installed FAIL']
20.3 4cpu: FAIL (22s) ['[S20.3] catalog and programs installed FAIL', '[S20.3] application launcher: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-launcher-4cpu', '[S20.3] catalog and programs installed FAIL']
20.10 2cpu: FAIL (27s) ['[S20.10] the Quick Settings button opens its panel FAIL', '[S20.10] quick settings: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-quick-settings-2cpu', '[S20.10] the Quick Settings button opens its panel FAIL']
20.10 4cpu: FAIL (25s) ['[S20.10] the Quick Settings button opens its panel FAIL', '[S20.10] quick settings: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-quick-settings-4cpu', '[S20.10] the Quick Settings button opens its panel FAIL']
20.11 2cpu: PASS (50s) 
20.11 4cpu: PASS (50s) 
20.12 2cpu: PASS (20s) 
20.12 4cpu: PASS (21s) 
20.14b 1cpu: PASS (57s) 
19.2 1cpu: PASS (38s) 
13.12 production 1cpu: PASS (23s) 
13.12 production 2cpu: PASS (7s) 
13.12 production 4cpu: PASS (6s) 
system-apps boot (desktop session, F12, smptest): PASS (50s) 
release: PASS (1227s) 
DONE
```

Reruns after the fixes:

```
20.1 1cpu: PASS (55s) 
20.1 2cpu: PASS (31s) 
20.1 4cpu: PASS (36s) 
20.2 1cpu: PASS (77s) 
20.2 2cpu: PASS (24s) 
20.2 4cpu: PASS (24s) 
20.3 1cpu: PASS (41s) 
20.3 2cpu: PASS (21s) 
20.3 4cpu: PASS (26s) 
20.10 1cpu: PASS (55s) 
20.10 2cpu: PASS (27s) 
20.10 4cpu: PASS (26s) 
20.11 1cpu: PASS (69s) 
20.15 1cpu: PASS (37s) 
DONE
```

**Negative controls**, each caught, on 1 CPU with the source restored byte
for byte:
- **F12 not intercepted.**
- **Console mode not holding the session back:** the key typed at the
  console never reaches the kernel shell.
- **Session without PowerControl:** the machine stays on.

## Limits

- **VFS file limit.** The VFS's 64 KiB per-file limit constrains the shell
  image, which now has about 4.6 KB of headroom.
- **Fixed-address shell.** WovenShell is still ET_EXEC, so it runs without
  ASLR. Building it as a position-independent executable would restore
  ASLR.
- **No terminal redraw.** The console starts on a cleared screen; earlier
  text is not redrawn.
- **Fixed hotkey.** F12 cannot be rebound.
