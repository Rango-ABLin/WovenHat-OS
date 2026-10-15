# Stage status

## Presentation and repaint performance (2026-10-15)

A fix to accepted Stage 19/20 behaviour, found by the Stage 20.7 fade gate.
On 1 CPU the fade showed only 4–6 of its frames, because every frame
over-drew. Three changes:
- **WovenShell.** It repaints up to four separate dirty areas instead of
  one bounding box. Window opening and notifications report their parts
  separately.
- **Damage.** Kernel `Damage` merges regions that form exactly a
  rectangle, so tiled repaints no longer overflow the list into a bounding
  box.
- **Present.** It converts each row once and copies it to the physical
  rows it covers; the virtio-gpu path does the same.

**Results.**
- A fading frame presents 52 k pixels instead of 167 k.
- The window-open frame dropped from about 1.8 G to 60 M cycles.
- 20.7 passed six consecutive 1-CPU runs.
- 20.1–20.14, 17.1–19.14 and 13.12 production pass on 1/2/4 CPUs.

See the
[performance audit](audit-presentation-performance-2026-10-15.md).

## Stage 20.14b — the desktop across two monitors (2026-10-15)

Roadmap Stage 18.9, Multi-monitor Desktop.
- **Compositor.** It drives a multi-head virtio-gpu, with heads side by
  side, and presents each damaged region to the heads it covers.
- **Syscall 207.** It reports the monitors.
- **Shell.** The taskbar and panels stay on the primary monitor; the
  wallpaper, pointer and window dragging span every monitor.

**Fixes found by the gate.**
- A drop on the second monitor no longer snaps back to the primary.
- Windows on the second monitor are no longer clipped.
- The scale toggle no longer resets the primary.

**Evidence.**
- **Gate `[S20.14B] desktop across monitors: PASSED`:** passes on 1/2/4
  CPUs. Host captures of both heads match the kernel's rendering after a
  window is dragged onto the second monitor.
- **Negative controls:** all five failed the gate as intended.
- **Regression:** 20.1–20.14a, 17.1–19.14 and the 13.12 production boot
  pass on 1/2/4 CPUs.

See the
[Stage 20.14b audit](audit-stage20-14b-desktop-monitors-2026-10-15.md).

**Stage 18 is complete in software.** Roadmap 18.1–18.9 are Stages 20.1
to 20.14b.

## Stage 20.14a — two monitors through virtio-gpu (2026-10-15)

Roadmap Stage 18.9, first part.
- **Transport.** A modern virtio 1.x PCI transport, `virtio_pci.rs`.
- **Driver.** A virtio-gpu 2D driver with several scanouts,
  `virtio_gpu.rs`. It creates, backs, binds, transfers and flushes one
  resource per head.

**Evidence.**
- **Gate `[S20.14] two monitors: PASSED`:** two heads show exact kernel
  patterns, and host captures of both heads match pixel for pixel on
  1/2/4 CPUs.
- **Negative controls:** all four failed the gate as intended.

See the [Stage 20.14a audit](audit-stage20-14a-monitors-2026-10-15.md).

**Next: 20.14b.** The compositor will present the desktop across the
heads, and the shell will use the second monitor.

## Stage 20.13 — workspaces (2026-10-15)

Roadmap Stage 18.8, Multiple Workspaces.

**Shell.** There are four workspaces, each with its own windows and
stacking order.
- **The switcher.** Four numbered boxes left of the Quick Settings button
  show them, with the current workspace highlighted.
- **Switching.** Clicking a box, or pressing F3, switches workspace. F4
  moves the focused window to the next workspace and follows it.
- **Per-workspace behaviour.** New windows open on the current workspace.
  The taskbar's window entries and tiling cover only the current workspace.
- **Implementation.**
  - The shell's stack holds only the current workspace's windows.
  - Switching saves that stack and restores the target's.
  - A window that closes, or whose owner exits, on a hidden workspace is
    removed from that workspace's saved stack.

**Keys.** F3 and F4 are new keys through WovenInput: PS/2 scancodes
0x3D/0x3E and USB usages 0x3C/0x3D. They reach userspace as input kinds 14
and 15 and libwoven's `Key::F3`/`Key::F4`. The kernel's own consumers
ignore them.

**USB F2 fix.** The USB keyboard map, from Stage 13.13, also lacked F2,
which is added.

**Gate changes.**
- The Stage 20.1 gate now checks the switcher, each box and digit exactly,
  and keeps checking the bare taskbar on both sides.
- The Stage 20.12 gate expects the signed-in name left of the switcher.

Gate `[S20.13] workspaces: PASSED`, driven by real i8042 input and checked
against the kernel's own rendering:
- Four workspaces, with the first current, and window A opens on it.
- F3 moves to workspace 2, and A and its taskbar entry disappear.
- Window B opens on workspace 2.
- Clicking box 1 brings A back and hides B.
- F4 moves A to workspace 2 and follows it, with A on top and both listed.
- F3 moves to workspace 3, which shows neither window.
- B is killed while hidden; back on workspace 2, only A remains.
- Killing the shell returns the display.

B lies inside A's shadowed footprint, so the gate judges "B hidden" by
B's colour being absent and the taskbar listing one window.

**Validation**, in the first worktree on `stage20-13`:
- **Lint and host tests:** kernel Clippy for production, every Stage 20
  feature, `stage13-13-test`, `stage13-16-test` and `stage19-12-test`, plus
  woven-shell and host Clippy, all with warnings denied, pass; the libwoven
  tests pass.
- **Gates:** 20.1–20.13 pass on 1/2/4 CPUs, and 13.13 and 13.6 pass on
  1/2/4 CPUs.

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. Each failed the gate as intended:
- Taskbar entries not filtered by workspace.
- Saved stack not restored on switching.
- F4 doing nothing.
- Switcher clicks ignored.
- Windows always opening on workspace 1.

Open:
- **No transition animation.** Workspace transitions are not animated yet.
- **No overview.** There is no workspace overview.
- **Fixed count.** There are always four workspaces.

## Stage 13.16 — tablets, pens, touch and game controllers (2026-10-14)

HID devices that describe their own reports now work.
- **Parser and decoder.** A pure report-descriptor parser and decoder
  (`kernel/src/hid.rs`) covers pointers, tablets, pens, touch screens and
  game controllers.
- **xHCI.** It fetches and parses each such device's descriptor.
- **Userspace.** Scroll, absolute-pointer, touch, pen and game-controller
  events reach userspace as input kinds 9–13.

**Evidence.**
- **Host tests:** pass against real descriptors for a tablet, a game pad,
  a pen and a touch screen. They caught a lost-axis bug.
- **Gate `[S13.16] report-protocol HID input: PASSED`:** QEMU's USB tablet
  delivers exact absolute positions, a click and wheel steps on 1/2/4
  CPUs.
- **Negative controls:** all four failed the gate as intended, one after a
  harness gap was closed.
- **Regression:** 13.5, 13.6, 13.7, 13.13 and 19.12 pass on 1/2/4 CPUs.

See the [Stage 13.16 audit](audit-stage13-16-hid-reports-2026-10-14.md).

**Open.** Multi-application devices, a virtio-input driver, and physical
qualification of digitizers and controllers.

## Stage 13.15 — WovenAudio playback for userspace (2026-10-14)

Applications can now play sound.
- **New capability.** `AudioPlayback` (24), which the permission broker
  can grant as `Permission::Audio`.
- **Syscalls.** 203 `AudioInfo`, 204 `AudioPlay` and 205
  `AudioSetVolume`, with libwoven wrappers.
- **Interrupt-driven HDA output.** The player sleeps until the stream's
  completion interrupt wakes it.
- **Ledger.** Every play attempt is recorded in the security ledger.

**Evidence.**
- **Gate `[S13.15] WovenAudio playback: PASSED`:** passes on 1/2/4 CPUs.
  QEMU's recording of the output holds the exact square wave the Ring-3
  client played, and a client without the capability is refused.
- **Negative controls:** all five failed the gate as intended.
- **Regression:** 13.8, 15.3, 15.4 and 15.8 pass on 1/2/4 CPUs.

See the [Stage 13.15 audit](audit-stage13-15-audio-2026-10-14.md).

**Open.** Streaming and mixing, other sample formats, recording through
`Microphone`, and physical codecs.

## Stage 13.14 — storage on NVMe and AHCI (2026-10-14)

A machine with no ATA disk now mounts its NVMe or AHCI disk at `/mnt`.
- `block_io` routes all primary storage I/O, the FAT32 mount, persistence,
  journals and swap to a selectable root device.
- A normal boot selects the first ready `nvme0` or `ahci0` after
  bring-up.
- ATA stays the default, so existing configurations are unchanged.

**Evidence.**
- **Gate `[S13.14] NVMe/AHCI storage: PASSED`:** passes for NVMe and for
  AHCI on 1/2/4 CPUs. A persisted file survives a remount and is found in
  the disk image on the host.
- **Production:** a production boot mounts the NVMe volume and reaches
  the shell, on 1/2/4 CPUs.
- **Negative controls:** all four failed the gate as intended.
- **Regression:** the ATA storage regression, the WovenFS device gate, and
  1-5, 10.8, 12.1, 13.3 and 13.4 pass on 1/2/4 CPUs.

See the [Stage 13.14 audit](audit-stage13-14-storage-2026-10-14.md).

**Open.** WovenFS on NVMe or AHCI, multiple volumes, interrupt-driven
NVMe and AHCI I/O, and physical-device qualification.

## Stage 13.13 — USB keyboards and mice into WovenInput (2026-10-14)

USB keyboards and mice now work in every normal boot.
- The xHCI driver enumerates each connected port, with its own slot per
  device.
- It binds an MSI-X or MSI vector through the driver manager.
- A worker sleeps until that vector fires, drains reports under the
  controller lock and publishes after releasing it.
- USB input reaches the same queue as PS/2.

**Two existing bugs fixed.**
- **MSI-X never delivered on any device.** Activation left Function Mask
  set.
- **The USB mouse Y axis was inverted.**

Gate `[S13.13] USB input: PASSED`. Input injected through QEMU's USB
keyboard and mouse arrives exactly, through device interrupts, with the
worker sleeping between them.
- **Runs:** passes on 1/2/4 CPUs.
- **Negative controls:** all six failed the gate as intended.
- **Regression:** 13.12 in all modes, and 13.2, 13.5, 13.6, 13.7, 13.9 and
  13.10, pass on 1/2/4 CPUs.

See the [Stage 13.13 audit](audit-stage13-13-usb-input-2026-10-14.md).

**Open.** Report-protocol HID devices (touchpads; Stage 13.16), USB
hotplug, a fuller keymap, and physical-device qualification.

## Stage 13.12 — boot-time driver bring-up (2026-10-14)

Until now, every Stage 13 driver ran only inside its own test build. A
normal boot contained no driver manager, NVMe, AHCI, xHCI or HDA code.
Every normal boot now runs `drivers::bring_up()`, which:
- matches each controller through the WovenDriver manager;
- binds its PCI function and MMIO BARs;
- initializes the controller at that address;
- rolls the binding back if initialization fails.

A missing or failing device never stops the boot. Ready drivers appear in
the device registry as `nvme0`, `ahci0`, `xhci0` and `hda0`.

**Evidence.**
- **Gate `[S13.12] driver bring-up: PASSED`:** passes in its three
  machine modes (all devices, AHCI without a disk, no devices) on
  1/2/4 CPUs.
- **Normal build:** a kernel built with no test feature binds all four
  drivers and reaches the shell, on 1/2/4 CPUs.
- **Negative controls:** four controls each failed the gate as intended,
  and the gate passed again once the source was restored.
- **Prior gates:** 13.1–13.10 pass on 1/2/4 CPUs.

See the
[Stage 13.12 audit](audit-stage13-12-driver-bring-up-2026-10-14.md).

**Stage 13 is not closed by this.** Still to do:
- mount storage from NVMe or AHCI;
- deliver USB keyboard and mouse input through an interrupt-driven xHCI
  worker into WovenInput;
- expose WovenAudio to userspace;
- move touchpad, touchscreen, pen and game-controller events out of the
  Stage 13.7 probe;
- physical qualification of Wi-Fi and Bluetooth.

## 2026-10-14 revalidation

The Stage 19 system applications now build outside the kernel, sign into WAPP,
and execute in Ring 3 through the existing verified application boundary.
WovenCalculator, WovenTerminal, and WovenFiles retain their bounded 1/2/4-CPU
acceptance; see their individual audits. The remaining roadmap applications,
catalog, and multi-package boot-image installation are now covered by the
ten-application release chain. WovenFiles now adds a read-only bounded
directory/file browser; see the
[WovenFiles audit](audit-stage19-files-2026-10-14.md).
WovenMonitor adds bounded read-only clock, display, and storage diagnostics
and passes signed 1/2/4-CPU execution; see the
[WovenMonitor audit](audit-stage19-monitor-2026-10-14.md).
WovenSettings adds bounded read-only identity, clock, and display-policy
diagnostics and passes signed 1/2/4-CPU execution; see the
[WovenSettings audit](audit-stage19-settings-2026-10-14.md).
WovenSecurity adds bounded read-only identity and security-ledger reporting and
passes signed 1/2/4-CPU execution; see the
[WovenSecurity audit](audit-stage19-security-2026-10-14.md).
WovenEditor adds bounded text preview and capability-enforced note writing and
passes signed 1/2/4-CPU execution; see the
[WovenEditor audit](audit-stage19-editor-2026-10-14.md).
WovenImages adds bounded image inventory and header inspection and passes
signed 1/2/4-CPU execution; see the
[WovenImages audit](audit-stage19-images-2026-10-14.md).
WovenUpdate adds bounded read-only update preflight and passes signed 1/2/4-CPU
execution; see the
[WovenUpdate audit](audit-stage19-update-2026-10-14.md).
WovenStore adds bounded read-only installed-catalog browsing and passes signed
1/2/4-CPU execution; see the
[WovenStore audit](audit-stage19-store-2026-10-14.md).

The complete `scripts/test-release.py` chain now passes on the current tree.
The debug QEMU exit path was corrected to issue the `isa-debug-exit` byte-width
write explicitly; the full chain then passed lint, host tests, 1/2/4-CPU debug
and release gates, all ten applications, catalog acceptance, and production
system-app boot. See the [release validation audit](audit-release-validation-2026-10-14.md).

This is software/QEMU qualification only; native hardware and destructive USB
media limits remain open.

The authoritative release chain now includes app-specific Clippy, the signed
ten-application catalog gate on 1/2/4 CPUs, and release packaging of WAPP
payloads, catalog descriptors, and per-artifact hashes. See the
[system-app release audit](audit-release-system-apps-2026-10-14.md). The
feature-gated release image now claims multi-package RAM installation; physical
media qualification remains open.

The `system-apps` production-image feature now installs the ten signed
packages during normal boot, with pre-mutation signature/ELF validation,
collision checks, and rollback of newly created VFS entries on publication
failure. It requires an explicitly provisioned application trust key; the
acceptance harness supplies the test key only for QEMU. The 4-CPU shell gate
confirmed installation before shell startup. Persistent media updates,
power-loss rollback, key rotation/revocation, and native hardware
qualification remain open. See the
[production system-app boot audit](audit-stage19-production-system-app-boot-2026-10-14.md).

Stage 14.5D was rerun on 1/2/4 CPUs. DHCP, DNS, ICMP, host-verified UDP/TCP,
SMP/TLB, and firewall transport/path checks passed with exit 33. See the
[Stage 14.5D revalidation audit](audit-stage14-5d-revalidation-2026-10-14.md).
The evidence remains QEMU virtio-only and does not claim native NIC/radio
qualification.

Stages 13.4, 13.5, 13.6, and 13.7 were independently rerun on 1/2/4 CPUs
and all passed with exit 33. See the
[Stage 13.4–13.7 validation audit](audit-stage13-4-7-validation-2026-10-14.md).
These QEMU gates validate software and protocol contracts, not native-device
interoperability. The Stage 13.6 fixture exercises USB HID keyboard support;
The bounded boot-mouse report decoder is now self-tested, but live xHCI mouse
polling/publication and physical USB mouse interoperability remain open. See
the [USB mouse decoder audit](audit-stage13-6-usb-mouse-decoder-2026-10-14.md).

Stage 16.1, 17.1, 18.1, 19.1, 19.3, and 19.10 were rerun on 1/2/4 CPUs
after the remote-termination cleanup change; all emitted their required pass
markers and exit 33. See
[the revalidation audit](audit-stage17-19-revalidation-2026-10-14.md).

The complete Stage 12 acceptance chain was independently rerun on the current
accepted tree. Stages 12.1 through 12.5 all passed on 1/2/4 CPUs with exit 33;
the retained evidence directories are `stage12.1-1cpu-1791977944719919100`,
`stage12.1-2cpu-1791978000197697900`,
`stage12.1-4cpu-1791978038096642600`,
`stage12.2-1cpu-1791978076763178400`,
`stage12.2-2cpu-1791978128405584500`,
`stage12.2-4cpu-1791978166570188900`,
`stage12.3-1cpu-1791978204787637600`,
`stage12.3-2cpu-1791978254549661600`,
`stage12.3-4cpu-1791978294758980400`,
`stage12.4-1cpu-1791978335142454100`,
`stage12.4-2cpu-1791978387714722500`,
`stage12.4-4cpu-1791978427671708800`,
`stage12.5-1cpu-1791978468908251500`,
`stage12.5-2cpu-1791978521938978000`, and
`stage12.5-4cpu-1791978563395332900`.

The ELF dynamic metadata boundary was hardened before shared-library work.
`DT_NEEDED` is now bounded and string-table validated, while the loader rejects
dependency-bearing images until dependency mapping, cross-image symbol scope,
dynamic TLS, and shared-image teardown exist. The Stage 11.3–11.5 1/2/4-CPU
matrix passed again with the new parser self-test; see the
[dynamic dependency metadata audit](audit-stage11-needed-metadata-2026-10-14.md).

## Host-test boundary repair — 2026-10-14

The freestanding kernel now has an explicit host-test boundary: test builds no
longer hide mutex/heap symbols needed by compiled production modules, and they
do not install a second panic or allocator handler. Warning-denying kernel
Clippy, all-targets Clippy, and the complete `wovenhat-os` integration-test
gate pass. The unsupported `cargo test --workspace --all-targets` command is
documented as a host-assembler mismatch rather than treated as a production
failure. See the [host-test audit](audit-host-test-boundary-2026-10-14.md).

## Stage 20.12 — sign-in screen and local accounts (2026-10-14)

Roadmap Stage 18.7, Login Manager.

**Accounts.** `libwoven::accounts` reads an accounts file with one local
account per line: `name:salt-hex:hash-hex`.
- The hash is `SHA-256(salt || PIN)`. It uses the same freestanding,
  software-only `sha2` crate as the kernel's boot measurement.
- No PIN is stored anywhere.
- Names are 1–16 printable ASCII characters other than `:`. Salts are
  1–32 bytes, and hashes exactly 32. Any other line is refused.
- Hashes are compared in constant time.
- Host tests check FIPS 180-2 vectors split across salt and PIN, and the
  parser's refusals.

**Shell.** With `--accounts <file>`, the desktop starts at a sign-in
screen. The screen shows "Sign in" and the accounts, with the chosen one
marked.
- Tab chooses the next account, digits enter its PIN, and Enter signs in.
  Masking and the three-try lockout work as on the lock screen.
- Once signed in, the account's name appears on the taskbar, left of the
  Quick Settings button.
- The lock screen then asks for that account's PIN, not `--pin`.
- If `--accounts` is given but no account can be read, the shell refuses
  to start rather than open the desktop to anyone.

**Fixes found by the first runs.**
- **The shell overflowed its user stack.** A user stack is 8 KiB
  (`UserStack::SIZE`). The desktop state was built in `main`'s 5.8 KiB
  frame, and the catalog loader's 2.3 KiB frame sat on top. Adding the
  accounts pushed the total past the guard page: a page fault at the
  shell's first instruction in `load_catalog`. The state now lives in a
  static (.bss), like the tile buffer, and the loaders fill it in place.
  `main` is down to 1.5 KiB and `load_catalog` to 1.1 KiB. The kernel's
  stack size is unchanged.
- **The gate tried to create `/etc`, which boot already creates.** It now
  accepts an existing directory.
- **The Stage 20.1 gate failed after Stage 20.10.** 20.10 put the Quick
  Settings button inside the strip the 20.1 gate required to be plain
  taskbar. CI's desktop-userspace job on `stage20-10` failed there. The
  20.1 gate now checks the strip on both sides of the button, and the
  button's fill and "Q" exactly. The fix is on `stage20-10` and merged
  forward.

Gate `[S20.12] sign-in: PASSED`. The kernel writes an accounts file with
`ada` (PIN 1234) and `lin` (PIN 9876), salted and hashed with the kernel's
own SHA-256. The shell runs with `FileRead` and real i8042 input. Screens
are compared row by row with the kernel's rendering.
- The desktop opens at the sign-in screen, with ada chosen.
- Tab chooses lin.
- ada's PIN is refused for lin.
- lin's PIN signs in, and lin's name is on the taskbar.
- Locked through Quick Settings, the lock screen refuses ada's PIN and
  takes lin's.
- Killing the shell returns the display.

Validation in the second worktree, on `stage20-12`:
- **Lint and tests:** kernel Clippy for every Stage 20 feature, host
  `--all-targets` Clippy and woven-shell Clippy, all with warnings denied,
  pass. libwoven host tests pass (16).
- **Gates 20.2–20.12:** pass on 1/2/4 CPUs.
- **Gate 20.1:** failed on 1/2/4 CPUs before the Quick Settings fix
  above. After the fix, it was rerun on 1/2/4 CPUs; see the commit.

Negative controls, each run on 1 CPU, with the source restored byte for
byte afterwards and the gate passing again:
- Any PIN accepted: fails at "ada's PIN is refused for lin".
- Tab ignored: fails at "Tab chooses lin".
- Lock forgets the session: fails at "the lock screen refuses ada's PIN
  and takes lin's".
- No sign-in screen: fails at "the desktop opens at the sign-in screen".
- Name not shown: fails at "lin's PIN signs in; lin's name is on the
  taskbar".

Open:
- **Every process still runs with the same kernel credentials.** Per-user
  sessions need the kernel to start processes as another user.
- **Accounts are read from a file given at startup.** There is no account
  creation, password change, or protected credential store; the file's
  integrity rests on the filesystem.
- **A 4-digit PIN behind a salted fast hash protects only against casual
  access.** An attacker who can read the accounts file can try all 10,000
  PINs instantly. A slow KDF and longer secrets belong with a real
  credential store.

## Stage 20.11 — lock screen (2026-10-14)

Roadmap Stage 18.6, Lock Screen.

**Shell.** Quick Settings gains a fourth row, "Lock screen", which locks
the desktop.
- **What locking hides.** No window is drawn, no keystroke reaches an
  application, and a notification is remembered and counted but never
  shown. The pointer still moves, unseen.
- **What it shows.** The wallpaper dimmed (160/255), with "Locked", the
  time, a prompt, and one masked dot per digit typed, all centred.
- **Unlocking.** The PIN is given with `--pin <digits>`, 1–16 digits.
  - Attempts are compared in constant time, and typed digits are wiped
    after each one.
  - A wrong PIN says "Wrong PIN".
  - Three wrong PINs in a row lock entry out for 5 s, during which even
    the right PIN is refused ("Try again shortly").
  - Without `--pin`, Enter unlocks.

**Why the PIN is not in the secret vault.** The PIN is in the shell's own
memory, not the kernel's secret vault. The vault needs the `Secrets`
capability. Codex's security model (Stage 15.3) reserves it for
applications the permission broker grants it to, and keeps it out of the
system-service ceiling, and the shell is a system service, because it
needs the display and input. Widening that ceiling to pass a gate would
be wrong, so this stage does not. Real accounts and credentials belong
with the login manager (18.7).

**Gate changes.**
- The Stage 20.10 gate now expects the fourth row, and passes.
- In the 20.11 gate, the lockout check first raced the shell. It began
  its one-second stability check while the typed digits were still
  showing. It now waits until the shell's input queue is empty and the
  lockout screen has settled.

Gate `[S20.11] lock screen: PASSED`. The shell runs with `--pin 2468` and
no more authority than the other desktop gates; real i8042 input. The lock
screen's rows are compared with the kernel's rendering: the dimmed
wallpaper and the text in the kernel font.
- Quick Settings locks the screen, and the window is hidden.
- `x` while locked reaches no application: A, waiting for `x`, is still
  running a second later.
- Two digits show as two dots.
- A wrong PIN says so and stays locked.
- 2468 unlocks, the window returns, and A then gets `x` and exits.
- After locking again, three wrong PINs lock entry out, and 2468 is
  refused for a full second. After the 5 s lockout, 2468 unlocks.
- Killing the shell returns the display.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.11-negative-control-*`:
- With the PIN check bypassed, the gate fails at "a wrong PIN says so and
  stays locked".
- With keys passed through while locked, it fails at "a key while locked
  reaches no application".

Open:
- **No idle auto-lock.** There is also no lock key beyond Quick Settings.
- **The PIN is a startup argument held in the shell's memory.** There is
  no credential store or account model yet; that is 18.7.
- **Nothing is protected against a process that can already read the
  shell's memory.**

## Stage 20.10 — Quick Settings (2026-10-14)

Roadmap Stage 18.5, Quick Settings. The roadmap lists Wi-Fi, Bluetooth,
sound, brightness, power, VPN and security. Most of these have no service
userspace can act on yet. This stage offers only toggles that act on the
system today, and the others are not faked.

**Shell.** A "Q" button left of the tray's last slot opens Quick Settings,
a translucent panel above the taskbar's right end. Like the other panels
it is drawn as a blended background plus a keyed overlay, and it, the
launcher and the Notification Center close each other. Its toggles:
- **Display scale, 2× or 1×.** The compositor surface is reallocated at
  the new geometry (syscall 117). Every window is kept on screen, and
  re-tiled if tiling is on. The pointer is clamped, and the whole desktop
  is repainted.
- **Do Not Disturb.** Notifications are still remembered and counted by
  the Notification Center badge, but raise no toast.
- **Tiling.** The same switch as F2.

hello-gui `--desktop-client` gains a `q` variant, Quiet, which posts a
notification each time it is sent `p`.

Gate `[S20.10] quick settings: PASSED`. Real i8042 input; each panel state
is compared with the kernel's own rendering over what lies beneath.
- The button opens the panel: scale 2×, Do Not Disturb off, tiling off.
- With Do Not Disturb on, `p` makes Q post a notification. The badge reads
  1, and the toast area stays clear for a full second.
- With tiling on, the only window fills the work area under the panel.
- Scale 1× makes the desktop 1280×800, and the panel follows the new
  geometry. Scale 2× from the panel at that geometry restores 640×400.
- The button closes the panel, Q takes its key and exits, and killing the
  shell returns the display.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.10-negative-control-*`:
- With Do Not Disturb ignored, the first version of the gate still
  passed. It checked the toast area once, at the instant the badge
  appeared. The shell paints the desktop layer, with the badge, before the
  toast layer within one repaint, so that instant can come before the
  toast is drawn. The gate now requires the toast area to stay clear for
  a second, and the control then fails at "a notification under Do Not
  Disturb is counted but raises no toast".
- With the scale toggle stuck, it fails at "display scale 1x makes the
  desktop 1280x800".

Open:
- Wi-Fi, Bluetooth, sound, brightness, power, VPN and security toggles
  await services userspace can reach.
- At 1×, a window cannot be resized to the whole work area. Its owner
  needs the old and new surfaces at once, and together they exceed the
  4 MB per-process budget, so the window keeps its size.
- Settings are not persisted.

## Stage 20.9 — Notification Center (2026-10-14)

Roadmap Stage 18.4, Notification Center.

**Shell.**
- **History.** Every notification is kept after its toast expires: the
  last 8, newest first. Each is stored as one row, `source: text`, where
  the source is the sending application's window title ("app" for a
  sender without a window), cut to what fits.
- **Unread badge.** A 10×11 accent box just left of the clock counts the
  notifications that arrived since the Notification Center was last
  opened, up to 9.
- **Panel.** Clicking the clock, or the badge, opens the Notification
  Center: a translucent panel above the taskbar's right end, with a title
  row holding "Clear", one row per notification, or "No notifications".
  - Opening it marks everything read, and the badge goes.
  - Clear empties the history.
  - A click outside the panel, or on the clock again, closes it.
- **One panel at a time.** It and the launcher close each other.
- **Rendering.** It is drawn like the launcher: a region-blended
  background, with an overlay copied by the keyed blit.

hello-gui `--desktop-client` gains an `n` variant, Notes, which posts
three notifications 10 ticks apart.

Gate `[S20.9] notification center: PASSED`. Real i8042 pointer and
keyboard input, and every pixel compared with the kernel's own rendering
(the background blended over the wallpaper, the border, title row and text
in the kernel font):
- Window N opens and posts "Build done", "Mail: 3 new" and "Battery 80%".
- The toast shows the latest, and the badge reads 3.
- A click on the clock opens the center, listing `Notes: Battery 80%`,
  `Notes: Mail: 3 new` and `Notes: Build done` in that order, and the
  badge goes.
- Clear leaves "No notifications", and a click on the clock closes the
  panel, showing the wallpaper again.
- N takes its key and exits, and killing the shell returns the display.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.9-negative-control-*`:
- With notifications not remembered, the gate fails at "clicking the
  clock opens the center, newest first, and clears the badge".
- With the badge left set on opening, it fails at the same check.

Open:
- No per-notification actions: a click on a row does not focus the
  sender.
- The history is not persisted across shell restarts.
- The kernel's own lifecycle notifications (`NotificationPoll`) are not
  shown.

## Syscall numbering for the desktop (2026-10-14)

Twice, the desktop's compositor syscalls took numbers that work on
`phase2` took at the same time, and merging conflicted:
- region blend and keyed blit at 122–124 collided with permissions, the
  audit ledger and secrets (122–129);
- the same calls renumbered to 130–132 collided with threads (130–135).

They now live at **200–202**:

| Number | Syscall |
|---|---|
| 200 | `GraphicsCompositorBlendRegion` |
| 201 | `GraphicsCompositorBlendRegionShared` |
| 202 | `GraphicsCompositorBlitKeyed` |

New desktop and compositor syscalls take numbers from the 200 block, so
the two lines of work cannot collide. The syscalls the desktop added
earlier, 116–121, were never contested and keep their numbers.

## Stages 20.4–20.8 — validation after rebasing onto phase2 (2026-10-14)

The Stage 20.4–20.8 commits were rebased onto `phase2` at `c5b13392`,
which includes the Stage 1–19 hardening (`b4f00fd3`) and the Stage 13.2
merge. They were then validated as one stack in an isolated worktree.
- Kernel Clippy is clean across all 77 feature configurations, including
  `stage13-2-test`, which compiles again.
- Clippy for woven-shell, hello-gui, libwoven and the host is clean, and
  the host tests pass.
- **102 QEMU gates pass across 1, 2 and 4 CPUs**:
  - every Stage 20.1–20.8 desktop gate;
  - all Stage 19.x application gates;
  - the kernel runtime stages back to 1–5;
  - the legacy-IRQ memory suite;
  - Stage 6 hotplug on 2 and 4 CPUs.
- In the rebase, syscalls 122–124 were renumbered to 130–132, and later
  to 200–202 (see "Syscall numbering for the desktop" above); see Stages
  20.5 and 20.6.

## Stage 20.8 — tiling: resize protocol, snap to edges, master and stack (2026-10-13)

Roadmap Stage 18.3, Window Experience: intelligent tiling. This completes
the design's 20.4–20.8 sequence. Workspace transitions are deferred to
roadmap 18.8.

**Input.** F2 is decoded (scancode 0x3C) and reaches applications.
- `woven_input::Key::F2` is encoded as input kind 8 and decoded by
  libwoven.
- The kernel's own terminal paths ignore it.
- This is an additive change to the `InputPoll` encoding.

**libwoven `layout`.** Pure geometry, host-tested:
- `tile(count, area)` places master and stack: the first window takes the
  left half, and the rest share the right half in equal rows, the last
  taking the remainder.
- `half(area, right)` gives the left or right half.
- `Frame::client` gives the client area under a title bar.
- The tests show frames cover the area exactly, with no overlap, for 1 to
  10 windows (capped at 8), and check the halves.

**Resize protocol.** The shell sends the owner `WSIZ` with a client size,
and the owner answers `WRSZ` with a new surface of that size (token,
width and height, owner-checked like `WOPN`), then releases the old one.
- The shell moves the window at once and swaps the surface when the reply
  arrives.
- For one second after a request, a vanished surface does not count as an
  exit: the owner may release the old surface before its reply is
  processed.
- hello-gui `--desktop-client` answers `WSIZ`.

**Shell.**
- Dropping a dragged window on the left or right screen edge snaps it to
  that half.
- F2 toggles tiling: master and stack over the screen above the taskbar,
  in window order, applied again whenever a window opens or closes.

Gate `[S20.8] tiling: PASSED`, driven by real i8042 input:
- Window A is dragged by its title bar onto the left edge and dropped. It
  snaps and is resized: its 320×362 client area and its title bar are
  exact.
- F2 tiles: A keeps the left half and B takes the right half, resized,
  with both title bars exact (A focused, B not).
- F2 turns tiling off. Both applications take their keys and exit, and
  killing the shell returns the display.

The first run failed at the tiling check because of the gate, not the
shell. A, focused after its drag, correctly casts its deep shadow across
the first 8 columns of B beside it, and the gate had expected plain blue
there. The gate now applies A's shadow bands to B's expected pixels, the
same stacking the Stage 20.5 gate computes.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.8-negative-control-*`:
- With F2 ignored, the gate fails at "F2 tiles the windows: master on the
  left, stack on the right".
- With snapping disabled, it fails at "dropping A on the left edge snaps
  it to the left half, resized".

Open:
- No minimum sizes: windows are never refused a tile size.
- No resizing by dragging window edges; resizing happens only through
  snapping and tiling.
- No keyboard tiling beyond F2. Real modifier keys (Ctrl, Alt, Super) are
  still missing.

## Stage 20.7 — physics-based animation: windows fade in on a spring (2026-10-13)

Roadmap Stage 18.3, Window Experience: smooth physics-based animation.

**libwoven `Spring`.** The spring eases a value toward a target, stepped
once per frame.
- It is fixed-point (16 fractional bits): no floating point, and every run
  is identical.
- Stepping is semi-implicit Euler with stiffness 0.08 and damping 0.5 per
  frame. That keeps both roots of the discrete system real and positive:
  the value approaches its target monotonically, with no overshoot, and
  settles within about 30 frames of a 255-step move.
- `step(skipped + 1)` keeps motion on time when frames are dropped.
- Host tests cover:
  - monotone approach with no overshoot, for three moves including
    negative values, settling within 60 frames, with at least 10
    intermediate values (a motion, not a jump);
  - stepping 7 frames at once equals 7 single steps;
  - determinism;
  - rest staying at rest, and retargeting starting from the current value.
- An integer Python model of the same arithmetic agreed before the first
  build: 28–30 frames, monotone.

**Shell.** A new window fades in: its opacity follows the spring from 0 to
255.
- Its title bar is drawn into the staging surface and region-blended at
  that opacity.
- Its client area is region-blended with the shared-surface blend (123).
- Its shadow bands are scaled to `opacity * alpha / 255`.
- While anything animates, the frame clock runs at 20 ms (2 ticks); at
  rest, at 250 ms.

Gate `[S20.7] spring animation: PASSED`. The kernel samples one pixel of
the appearing window as fast as it can.
- It samples the **physical framebuffer**, which changes only when a fully
  composed frame is presented. The compositor surface is updated layer by
  layer, and sampling it could catch a frame half built.
- For every opacity `a` it computes what that pixel must read: the
  wallpaper, the window's own shadow band at `48a/255`, then the client
  colour blended at `a`.
- Every sample must be one of those values, the opacities they imply must
  never decrease, at least three intermediate opacities must be seen, and
  the pixel must end fully opaque.
- Then A takes its key and exits, and killing the shell returns the
  display.

The first run sampled 11 distinct frames over 69 ticks, at opacities
0, 20, 80, 135, 178, 217, 244, 248, 251, 253 and 255: the spring's curve.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.7-negative-control-*`:
- With windows appearing at full opacity, only opacities 0 and 255 are
  seen, and the gate fails at "the window passes through at least three
  intermediate opacities".
- With the shadow not scaled by the fade, the samples are no longer
  explained, and it fails at "every sample is the window blended at some
  opacity, never decreasing".

Open:
- Only window opening is animated. Closing cannot fade, because the
  surface is gone when the owner exits. Minimise does not exist yet.
- The spring constants are fixed.

## Stage 20.6 — translucent overlays over windows (2026-10-13)

Roadmap Stage 18.3, Window Experience: subtle translucency.

**Kernel: keyed blit.** Syscall 202 (first numbered 124, then 132; renumbered on
rebasing onto the Stage 1–19 hardening), `GraphicsCompositorBlitKeyed`, is the
region blit of one of the caller's own surfaces, except that source pixels
equal to `display::TRANSPARENT_KEY` (magenta, `0x00FF00FF`, which no
WovenHat chrome uses) leave the destination as it was.
- It uses the same region snapshot and authority as the other region
  blits.
- `display::Surface::blit_region_keyed` does the work, with two new host
  tests: key skipped and everything else copied; without keyed pixels,
  exactly the region blit, clipping included.
- libwoven adds `TRANSPARENT` and `GraphicsSurface::present_region_keyed`.

**Shell.**
- The launcher panel and the toast are translucent, 216/255, over whatever
  lies beneath, windows and shadows included. Their background is
  region-blended. Their border, title row, selection, and text are an
  overlay drawn on the transparent key and copied with a keyed blit.
- The pointer is now an overlay too. It no longer reads the screen back
  pixel by pixel through `screen_pixel`, which cost about 70 system calls
  per pointer repaint.

**Gate changes.** The Stage 20.2 and 20.3 gates now expect the toast's and
the panel's backgrounds blended over the wallpaper. This is an intended
change of appearance; both pass.

Gate `[S20.6] translucent overlays: PASSED`. The kernel recomputes each
pixel: what lies beneath (wallpaper, the focused window's shadow bands, or
the window), the background blended over it, and the overlay rendered with
its own font and outline.
- A's toast is translucent over the wallpaper.
- A is dragged onto the taskbar's edge. The pointer rests over A's client
  area: its outline and fill are exact, and its transparent cells show A.
- F1 opens the empty launcher over A. The panel is translucent over A's
  client area, A's shadow and the wallpaper, every pixel except the
  pointer's cells, and the pointer stays above it.
- The panel closes, A takes its key and exits, and killing the shell
  returns the display.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.6-negative-control-*`:
- With overlays copied without the key, the gate fails at "the toast is
  translucent over the wallpaper", because the key colour lands.
- With an opaque panel background, it fails at "the launcher panel is
  translucent over a window and its shadow".

**Physical hardware check.** On 2026-10-13 the Stage 15.2 physical WovenFS
qualification ran again on the USB flash drive. The drive is identified by
model `VendorCo ProductCode USB Device`, 8,052,549,120 bytes, and serial
5474001281346643368; the system NVMe disk was never addressed. The run
used the Stage 20.5 commit, in an isolated worktree, with the device
passed raw to the kernel's ATA driver.
- Formatting, writing, remounting, VFS mount, reading, unlink, and unlink
  surviving a remount passed on 1, 2 and 4 CPUs, each with exit code 33.
- Windows then read `WOVENFS1` at LBA 0 with checkpoint sequence 5.
- Evidence is in the worktree's `audit-artifacts/wovenfs-physical-*`.
- This shows the storage stack still works on real flash with the desktop
  stages in place. It is not a native boot of the desktop on physical
  hardware.

Open:
- Translucency is uniform per layer. There is no blur behind translucent
  surfaces.
- Adaptive surfaces (chrome tinted from the wallpaper) were not done in
  this stage. They wait for selectable wallpapers, since there is
  currently one fixed wallpaper.

## Stage 20.5 — depth: window shadows and elevation (2026-10-13)

Roadmap Stage 18.3, Window Experience: dynamic depth.

**Kernel: region blend.** Syscalls 200 and 201 (first numbered 122/123, then 130/131;
renumbered on rebasing onto the Stage 1–19 hardening, which took 122–129),
`GraphicsCompositorBlendRegion` and its shared variant, are the region blit
(120/121) with a uniform opacity in place of the copy. They use the same
region snapshot, so only the exposed part is copied, and the same authority
rules.
- Four values have to fit in three registers. The destination word packs x
  and y as 24-bit signed fields, ample for a compositor bounded at
  1920×1200, with the opacity in the top byte.
- The byte between them is reserved and must be zero; anything else is
  refused.
- libwoven adds `GraphicsSurface::present_region_blended`,
  `compositor_blend_region_shared` and `pack_blend_destination`, with a
  host test of the packing.

**Shell.**
- Every window casts a soft shadow: two bands of black blended over what
  lies beneath, offset down and right, outer band lighter.
- The focused window sits higher: bands at (8 px, 24/255) and
  (4 px, 48/255), against (4, 20) and (2, 40) for the others. Focus changes
  repaint both windows' shadows.
- Shadows are clipped at the taskbar.
- Repaint areas follow a window's footprint (its frame plus its deepest
  shadow), not just its frame, so opening, closing, moving and refocusing
  leave no stale shadow.
- Shadow bands are rendered as solid chunks in the staging surface and
  region-blended into the exposed rectangle.

Gate `[S20.5] window depth: PASSED`. Every shadow pixel checked is
recomputed by the kernel: the wallpaper, then each window's bands, bottom
to top, with `display::blend_pixel`.
- With B focused, B's right and bottom strips carry the deep bands, and
  A's exposed strips carry the shallow ones.
- A shadow pixel really differs from the bare wallpaper.
- Clicking A's title bar swaps the shadows, including where both windows'
  bands overlap.
- Dragging A onto the taskbar's edge: its shadow beside it stops at the
  taskbar, and the accent line and taskbar rows below it are untouched.
- Both applications exit, taking their windows and shadows with them.
- Killing the shell returns the display.
- The first run showed the gate typing to B before the shell had seen A
  go. A key there goes to the still-focused, exited A, so the gate now
  waits for A's window to disappear first, as the Stage 20.2 gate does.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.5-negative-control-*`:
- With the focused window given the shallow shadow, the gate fails at
  "focused window B casts the deeper shadow".
- With shadows allowed onto the taskbar, it fails at "shadows stop at the
  taskbar".

Open:
- Shadows are rectangular bands, without blur or rounded corners.
- No dimming of unfocused windows yet.
- A key typed in the instant between an application exiting and the shell
  noticing, within one 250 ms frame, goes to the exited application and is
  lost.

## Stage 20.4 — windows that move and close; budgeted client surfaces (2026-10-13)

Roadmap Stage 18.3, Window Experience, first step: the prerequisite in
`docs/design-stage18-3-window-experience.md`.

**Surface budgets (kernel).** The 320×240 per-surface cap is gone.
- A surface may be up to 1920×1200, the compositor's own bound.
- Pixels are budgeted: one process may hold `OWNER_PIXEL_BUDGET` (1 M
  pixels, 4 MB) across its surfaces, and all processes together
  `SYSTEM_PIXEL_BUDGET` (4 M pixels, 16 MB).
- The existing counts stay: 4 surfaces per process and 16 in total.
- Going over a process's budget is `OwnerFull`, going over the system's is
  `SystemFull`, and a destroy returns its budget.
- The sizes are defaults pending the owner's decision on a desktop
  session's memory target.

Larger surfaces needed three changes to how the registry holds its lock:
- **The registry lock disables preemption, not interrupts.** Copying a
  9 MB surface must not hold interrupts off. Nothing takes the lock from
  interrupt context. The compositor already nests its preemption lock
  under the scheduler's interrupt lock on the exit path, so the same rank
  order holds.
- **Allocation and freeing happen outside the lock.** A new surface is
  allocated between a budget check before and a re-check after, so a
  create that lost a race frees its surface after unlocking. A destroyed
  surface is freed after unlocking. On the exit path, which runs under the
  scheduler lock, it is freed in place.
- **Region blits copy only the region.** The new
  `user_graphics::snapshot_region` copies just the exposed part of a
  surface, so a small repaint of a large window stays small. Previously
  every region blit copied the whole surface.

**Shell.**
- Windows may be as large as the desktop above the taskbar.
- A drag on a title bar moves the window, raised and focused. The frame is
  kept on screen and above the taskbar, and the old and new places are
  repainted on every motion step.
- Each title bar has a close control. A click on it sends the owner
  `WCLS`, without focusing the window. The window goes when the owner
  releases its surface, so an application can finish first or decline.
- hello-gui `--desktop-client` honours `WCLS`, and gains a `c` variant
  with a 400×260 window.

Gate `[S20.4] windows that move and close: PASSED`.
- The kernel checks the budgets directly:
  - oversized dimensions are `OutOfRange`;
  - a 1024×1024 surface fills a process's budget, so a further 1×1 is
    `OwnerFull`, and is admitted once the large surface is destroyed;
  - four processes with 1024×1024 each fill the system, so a fifth is
    `SystemFull`, and is admitted once one process releases.
- On the desktop, real i8042 input:
  - window A opens, then a 400×260 window opens on top with its title bar
    and close control exact;
  - A is dragged by its title bar by (300, 250). It lands at the computed
    place, above the wide window, focused, with its title bar and its
    damage patch exact. Its old place shows wallpaper, and the wide window
    shows where A left it;
  - the wide window's close control makes it exit 0, and it disappears;
  - A, still focused, takes `x` and exits;
  - killing the shell returns the display.
- Stage 20.2's title-bar checks were updated to include the close control,
  an intended change of appearance.

Negative controls, with evidence in
`audit-artifacts/stage20.4-negative-control-*`:
- With dragging disabled, the gate fails at "dragging the title bar moves
  the window and repaints both places".
- With the per-process budget lifted, it fails at "one process is held to
  its pixel budget".

Open:
- No resize: the protocol request is in the design. No minimise or
  maximise.
- Pointer events are still not forwarded to applications.
- The other compositor paths (whole-surface blit, blend) still copy whole
  surfaces, now under a preemption lock rather than an interrupts-off one.

## Stage 20.3 — WovenStart application launcher (2026-10-13)

Roadmap Stage 18.2, WovenStart: application launcher and search.

**Catalog.** `/share/applications/*.app` holds one small text file per
application, with a `Name=` line and an `Exec=` line. In the command, `%p`
becomes the shell's process id; that is how a launched application finds
the shell, which closes the discovery gap for launched applications. The
shell loads up to eight entries at startup and sorts them by name.
Directories, non-`.app` files and malformed entries are skipped. A missing
catalog, or no `FileRead`, leaves the launcher empty rather than stopping
the shell.

**Launcher.** The start button, or F1, opens a panel above the taskbar. It
has a title row, a search row showing the query, and one row per matching
application, the first highlighted.
- Typing filters by case-insensitive substring. Backspace edits, Tab moves
  the selection, Enter launches the selection, and F1 closes.
- A click on a row launches it; a click outside the panel closes it.
- While the panel is open, keys go to it rather than to the focused window.
- With no match, the panel says "No matches".
- The panel is a layer of its own in the exposed-region repaint, between
  the toast and the pointer.

**Launching and reaping.** The shell starts an application with
`SpawnCommand`, which needs `FileRead` and `ProcessCreate`. The application
is an ordinary user process with the default user capabilities, including
`Ipc` but not `DisplayIo`, and it is the shell's child.
- If the program cannot be started, the shell shows a "Could not start …"
  toast.
- `waitpid` does not block (it returns would-block while the child runs),
  so the shell polls its children every frame and reaps each as it exits.
  Without that, exited applications would stay zombies until the shell
  exited.
- libwoven adds `open`, `close` and `read_dir`. `read_dir` requires a
  255-byte name buffer, because the kernel copies the whole name.
- The kernel adds `task::child_count`, test-only, so the gate can see
  reaping.

**Fixed: double-encoded UTF-8 (since Stage 19.14).** The negative-control
runs for 19.14, 20.1 and 20.2 restored source files through Windows
PowerShell 5.1 `Get-Content`. It decoded UTF-8 as Latin-1 and wrote the
result back as UTF-8, so every `×` and `—` in doc comments in
`compositor.rs`, `libwoven/src/lib.rs` and `woven-shell` became mojibake,
and some was committed. No code was affected. The runs were repaired here.
Negative controls now edit and restore files byte for byte through Python.

Gate `[S20.3] application launcher: PASSED`. The catalog is Alpha, Beta and
Gamma, where Gamma names a missing program, plus a stray `notes.txt`. The
shell runs with `FileRead` and `ProcessCreate`, driven by real i8042 input.
Every panel state is compared with the kernel's own rendering.
- F1 opens the sorted catalog, without `notes.txt`. Tab moves the
  selection. Typing `be` filters to Beta.
- Enter launches Beta: its window opens, the panel closes, and the shell
  has one child. Beta takes `y` and exits, and the shell reaps it (child
  count 0).
- The start button opens the panel, and clicking Alpha's row launches it.
  Alpha takes `x` and exits, and is reaped.
- `g` then Enter shows "Could not start Gamma", with no child.
- `zz` shows "No matches", Enter does nothing, and F1 closes.
- Killing the shell returns the display.

Negative controls, with evidence in
`audit-artifacts/stage20.3-negative-control-*`:
- With reaping disabled, the gate fails at "Beta takes its key, exits, and
  is reaped".
- With the search ignoring the query, it fails at "typing filters the
  list".

Open:
- The catalog is read once at startup, with no reload, and up to 8 entries.
- No icons, no categories, and no search beyond names.
- Launched applications get the default user capabilities; there is no
  per-application manifest of capabilities.
- Not started at boot yet (see Stage 20.1).

## Stage 20.2 — desktop applications: windows, taskbar entries, tray, notifications (2026-10-13)

Roadmap Stage 18.1, Desktop, completed.

**Protocol.** Applications talk to woven-shell over IPC. Each message
starts with a four-byte tag:
- `WOPN`, token, width, height, title: open a window showing a surface the
  sender shared with the shell.
- `TRAY`, token: show a 16×16 surface in the system tray.
- `NOTE`, text: show a notification toast for 4 s.

A surface is accepted only if the sender owns it. `libwoven::surface_owner`
reads the owner from the token, and the kernel enforces that name on every
operation. A window or icon disappears when its surface does, normally
because its owner exited; the shell's per-frame damage check detects this.
Keystrokes go to the focused window's owner as one-byte messages.

**Shell.**
- Windows cascade from (40, 40) with a title bar above the client area,
  focused or unfocused, and never overlap the taskbar.
- The taskbar has one entry per window, highlighted when focused. Clicking
  an entry, or a window, raises and focuses it.
- Tray icons sit right to left from the clock.
- The toast is drawn top-right.
- The pointer is a two-tone arrow, hidden until the mouse first moves.
- The shell waits on input, messages and damage together, with a 250 ms
  frame clock.
- Without the `Ipc` capability it runs the desktop with applications
  disabled, rather than failing; this keeps Stage 20.1's minimal
  capability set working.

**Exposed-region redraw.** Every change becomes a repaint of one
rectangle, recomposed completely, bottom to top:
1. the desktop;
2. tray icons;
3. each window: its title bar, then its client surface;
4. the toast;
5. the pointer, drawn over what is already on screen.

Each layer contributes only its part inside the rectangle. This needs
syscalls 120, `GraphicsCompositorBlitRegion` (own surface), and 121,
`GraphicsCompositorBlitRegionShared`. They carry the same authority as the
whole-surface blits and are backed by `display::Surface::blit_region`, with
two new host tests. A whole-surface blit would paint over windows stacked
above outside the rectangle.

Client damage still goes through `GraphicsCompositorBlitDamage`. That blit
can land over a higher window, so the shell repaints the damaged rectangle
before presenting, and the overdraw never reaches the screen.

**Test-only change.** The i8042 loopback helpers (`mouse::loopback`,
`keyboard_loopback`) are now compiled for this gate too.

Gate `[S20.2] desktop applications: PASSED`. The shell runs with two
hello-gui `--desktop-client` applications, driven by real i8042 pointer
packets and keyboard scancodes.
- Application A opens a window. The kernel checks against its own
  rendering:
  - the window's frame, and the focused title bar with the kernel font;
  - A's taskbar entry;
  - the green tray icon;
  - the toast, with its border.
- A redraws a patch and reports damage, and the patch appears.
- B opens on top and focused: B covers A at the overlap, A's title bar
  turns unfocused, and both entries show the right state.
- The toast expires and the wallpaper shows through.
- The pointer moves onto A's entry and clicks: A is raised. The pointer is
  drawn at its position.
- `x` is typed: A receives exactly its key and exits 0. Its window, entry
  and tray icon disappear, and B is focused.
- A click in B, then `y`: B receives its key and exits, leaving the desktop
  empty.
- Killing the shell returns the display to the terminal.

The Stage 20.1 gate still passes with the extended shell.

Negative controls, with evidence in
`audit-artifacts/stage20.2-negative-control-*`:
- With keys sent to the bottom window instead of the focused one, the gate
  fails at "key x routed to focused window A".
- With taskbar entries ignoring clicks, it fails at "taskbar click raises
  window A".

Open:
- **Discovery.** Applications learn the shell's process id from their
  arguments; there is no name service yet.
- **Window controls.** No move, resize, close button or minimise. Windows
  are limited to the 320×240 surface cap, and the shell takes four windows
  and four tray icons.
- **Notifications** are one toast at a time; there is no history. The
  Notification Center is 18.4.
- **Input routing.** Only printable ASCII keys are forwarded. Pointer
  events are not forwarded to applications yet.
- **Not started at boot yet** (see Stage 20.1).

## Stage 20.1 — WovenShell desktop: wallpaper, taskbar, clock (2026-10-13)

Roadmap Stage 18.1, Desktop, first part. Repo stage 20 is roadmap Stage 18,
WovenShell. Stages 18–19 of this repo were roadmap Stage 17, WovenCompositor.

**Desktop program.** `apps/woven-shell` is a new userspace program, a
separate crate built like hello-gui.
- It takes the compositor lease, and switches to display scale 2 on a
  screen of at least 1280×800 so the 5×7 font stays legible.
- It draws:
  - a procedural wallpaper: a vertical gradient with a faint woven grid;
  - a translucent taskbar with an accent line;
  - the WovenStart button, a placeholder until 18.2;
  - a clock (`HH:MM`, or `HH:MM:SS` with `--seconds`).
- It wakes four times a second through `FrameClock` and repaints only the
  clock's tiles when the text changes. It drains input, since nothing on
  the desktop takes input yet.

Client surfaces are capped at 320×240, so the desktop is never one surface.
The scene is a pure function of position and state. The shell renders it
in software into a `libwoven::Canvas`, one 320×40 tile at a time, uploads
each tile into one staging surface and blits it into place. Any region is
repainted by rendering the tiles that cover it.

**Kernel.**
- `kernel/src/rtc.rs`: CMOS real-time clock.
  - Readings are taken only while update-in-progress is clear, and accepted
    when two consecutive readings agree.
  - BCD and binary, and 12- and 24-hour forms, are decoded. Invalid digits
    and out-of-range fields are refused.
  - It is serialised by its own lock, with bounded polling. Times are UTC,
    the clock's own zone.
- Syscall 119, `ClockRead`: returns the packed date-time. It needs no
  capability, like `Getticks`.
- Syscall 118, `GraphicsSurfaceWrite(handle, rect, pixels)`: uploads
  little-endian pixels into one of the caller's surfaces.
  - The rectangle is checked against the surface before anything is copied,
    which bounds the copy by the surface caps.
  - Pixels are copied from user memory with no lock held, and the handle is
    re-resolved for the write, so a surface destroyed in between is refused.
  - The unused high byte is cleared.
  - `display::Surface::write_pixels` does the write, with two new host
    tests.

**libwoven.** `GraphicsSurface::write` and `write_canvas`, `Canvas`
(clipped fill, blend, shade and text), `blend`, `clock` / `DateTime`, and
`font`. The font is the kernel's own source file, so client text matches
kernel text pixel for pixel.

**Defects found and fixed.**
- **Kill path skipped display teardown (since Stage 18).** A process
  killed from outside is torn down by `complete_process_termination`, not
  `exit_current_process`. That path never released the process's client
  surfaces or its compositor lease, so a killed compositor or desktop kept
  the screen forever. Both are now released there, under the same
  scheduler lock as on the exit path.
- **App linker script missed large-model sections.** The large code model
  emits `.ltext`, `.lrodata`, `.ldata` and `.lbss`, which `link.x` did not
  match. The linker placed them as orphans in the first, read-execute
  segment, so the shell's first write to a static faulted. No earlier app
  had a writable static. The fix is in all three apps.
- **Stale images.** Each app now has a `build.rs` that reruns when
  `link.x` changes, because Cargo does not track files named only in link
  arguments.

Gate `[S20.1] woven-shell desktop: PASSED`. QEMU's 1280×800 framebuffer
gives a 640×400 desktop at scale 2.
- The kernel checks the clock decoder against fixed vectors, then reads the
  hardware clock.
- It starts the shell as a system service (`Console`, `DisplayIo`,
  `InputIo`) and waits for it to take the display.
- It checks the geometry against the framebuffer and the scale rule.
- It checks three 40×40 wallpaper blocks against the formula restated in
  the kernel.
- It checks the accent line and the translucent taskbar body.
- It checks the start button against the label rendered with the kernel's
  own font.
- It checks the clock region against the kernel's own reading of the
  hardware clock, allowing up to 2 s of lag, then waits for a later
  reading, so the clock is seen to advance.
- It reads the physical framebuffer back at scale 2.
- It kills the shell, then checks that the lease is gone, presentation has
  stopped, and terminal text reaches the framebuffer again.

Negative controls, with evidence in
`audit-artifacts/stage20.1-negative-control-*`:
- With the kill-path teardown removed (the pre-fix behaviour), the killed
  shell keeps the display and the gate fails at "display returned to the
  terminal".
- With the shell's clock never repainting, it fails at "clock advances
  with the hardware clock".

Open:
- **Not yet started at boot.** The desktop runs under its acceptance gate.
  Starting it at boot belongs with session start: the login manager (18.7),
  or a boot option before that.
- **18.1 items still to come:** client windows on the desktop with a
  taskbar window list, the system tray, and notification toasts.
- **WovenStart** is a placeholder button; the launcher is 18.2.
- **Clock** shows UTC: there is no time-zone configuration.
- **Kill-path cleanup now matches explicit exit.** Scheduler-driven remote
  termination releases thread handles, queued notifications, terminal
  foreground ownership, and process sockets before publishing the zombie.
  Host Clippy and the complete host test suite pass after this fix; physical
  USB and multi-monitor qualification remain outside the QEMU gate.

## Stage 19.14 — integer display scaling and paced animation (2026-10-14)

Roadmap Stage 17's DPI-scaling and animation items. This closes the
compositor stage except multiple monitors, which is deferred (see Open).

**Scaling.** The compositor keeps an integer display scale, 1 to
`MAX_SCALE` (4).
- The surface is the framebuffer divided by the scale, clamped to the
  existing 1920×1200 bound. `display::present_scaled` draws each surface
  pixel as a scale×scale block, and scales the damage with it. `present`
  is now `present_scaled` at scale 1, so every earlier presentation path
  is unchanged. Three new host tests in `tests/display_compositing.rs`
  cover:
  - block expansion;
  - damage scaling;
  - screen clipping, plus scale 1 matching `present` byte for byte.
- Changing the scale reallocates the surface, cleared to black and wholly
  damaged; the holder redraws at the new geometry. The allocation and the
  free happen with the compositor lock released, and the lease is
  re-checked afterwards because the holder can exit in between.
- A new lease holder always starts at scale 1, whatever the previous holder
  chose. The reset happens in `claim`, not in `release_owner`, because the
  exit path holds the scheduler lock and must not allocate.

Two syscalls, both requiring `DisplayIo` and the lease:
- 116, `GraphicsCompositorInfo`: returns `width | height << 24 |
  scale << 48`.
- 117, `GraphicsCompositorScale(scale)`: returns the new geometry in the
  same packing, or ERROR for a scale outside 1–4.

libwoven adds `CompositorInfo`, `compositor_info` and
`compositor_set_scale`. Pointer motion stays relative; the window manager
tracks the pointer in surface coordinates.

**Frame pacing.** libwoven `FrameClock` paces at a fixed period on the
kernel timer, with no new kernel interface.
- Frame boundaries are absolute (`start + n * period`), so one late frame
  does not push back the rest.
- A late caller is told how many frames it skipped, so an animation stays
  on time rather than slowing down.
- `wait(mask)` uses the existing `WaitAny` deadline, so a window manager
  can sleep on input, messages or damage until the next frame. A mask of 0
  sleeps for the frame alone.

Gate `[S19.14] display scaling and frame pacing: PASSED`, on QEMU's
1280×800 framebuffer:
- The kernel checks that the boot compositor is at scale 1 and that scales
  0 and 5 are refused.
- It takes the display, sets scale 2 (640×400), and checks that the
  repaint writes all 1280×800 pixels. It blits an 8×8 surface at (10, 10)
  and reads the physical framebuffer back: the 16×16 block at (20, 20) has
  the colour, and its neighbours do not.
- The unprivileged probe is refused info and scale (exit 223 otherwise).
- hello-gui `--scaled-compositor`:
  - checks that info is refused before claiming and reads scale 1 after;
  - checks that out-of-range scales leave the geometry alone;
  - sets scale 2 and checks the exact half geometry, black corners, and an
    out-of-range read refused;
  - checks the first present writes the whole physical screen;
  - animates a square for 12 frames at 2 ticks per frame. No frame may
    arrive before its due tick, each frame writes exactly the 200×150
    backdrop region at 2×2, and the whole run takes at least 24 ticks.
    Observed: 12 frames in 25 ticks.
- The kernel then re-reads the final frame at scale 2 and checks that the
  next holder starts at scale 1.

Negative controls, with evidence in
`audit-artifacts/stage19.14-negative-control-*`:
- With presentation ignoring the scale, the gate fails at "rescale repaints
  the whole screen".
- With a `FrameClock` that never sleeps, hello-gui fails at step 176 (a
  frame arrived early).

Open:
- **Multiple monitors (deferred).** The firmware framebuffer is one
  output, and there is no GPU or virtio-gpu driver to enumerate more. The
  roadmap says native GPU support must not block the first GUI release, so
  this waits for a display-driver stage.
- Scale is whole-number only, and is not detected from the display: there
  is no EDID or physical-size source.
- No vsync. Frames follow the 100 Hz timer, so the shortest period is
  10 ms and frames are not aligned to the display's refresh.
- Rescaling transiently holds two surfaces (up to about 18 MB). This is
  bounded because only the holder can rescale.

## Stage 19.13 — translucent windows and shadows (2026-10-14)

Roadmap Stage 17's transparency and shadow items.

Pixels carry no alpha channel (`0x00RRGGBB`), and per-pixel alpha would have
made every existing surface, whose high byte is zero, transparent. So
translucency is a property of the blit. `display::Surface::blend` draws a
source with uniform opacity, using `display::blend_pixel`: per channel,
`(over * a + under * (255 - a) + 127) / 255`. That is exact at both ends:
opacity 0 changes nothing and reports no damage, and 255 is a plain blit.
Clipping matches `blit`. Three new host tests in
`tests/display_compositing.rs` cover:
- endpoints, rounding at the midpoint, and colour-over-itself identity;
- opacity 255 matching `blit` pixel for pixel, and opacity 0 touching
  nothing;
- clipping off the corner, so only the overlap is mixed.

Two syscalls carry the opacity in their third argument. The existing blits
(102, 110) ignore that argument, so it could not be repurposed without
breaking their ABI:
- 114, `GraphicsCompositorBlend`: one of the caller's own surfaces. This is
  how a window manager draws shadows and translucent chrome.
- 115, `GraphicsCompositorBlendShared`: a shared client window.

The authority is the same as the corresponding blits: `DisplayIo` plus the
lease, and the caller's own token for 114 or a grant naming the caller for
115. An opacity above 255 is refused rather than truncated. libwoven adds
`GraphicsSurface::present_blended` and `compositor_blend_shared`.

Gate `[S19.13] translucency and shadows: PASSED`:
- The probe is refused both blends.
- hello-gui `--alpha-compositor` draws an opaque backdrop, a black shadow
  from its own surface at 96/255 offset under the window, and the Stage 19.7
  `--client`'s shared window at 160/255 over both.
- It checks four regions against the formula, restated independently in the
  app: backdrop only, shadow only, window over the backdrop, and window over
  the shadow.
- It checks that opacity 0 leaves the screen unchanged, that the private
  surface is refused, and that opacity 256 is refused.
- The kernel recomputes the four regions with its own `blend_pixel` after
  both processes exit.

Local 1/2/4-CPU evidence is recorded in the
[Stage 19.13 transparency audit](audit-stage19-13-transparency-2026-10-13.md).

Negative control: with `blend` copying opaquely, the compositor fails its
region check (exit 167) and the gate fails. Evidence is in
`audit-artifacts/stage19.13-negative-control-*`.

Open:
- No per-pixel alpha, so no anti-aliased edges or soft shadows; a soft
  shadow needs several stacked blends.
- No animation: frame timing belongs to the window manager, and there is no
  vsync source.

## Stage 19.12 — PS/2 pointer input; keystrokes delivered at IRQ time (2026-10-14)

**Defect fixed (Stages 19.5/19.6).** The keyboard IRQ handler only queued raw
scancodes. They became WovenInput events only when a consumer called
`keyboard::poll` or `read_bytes`, which in practice meant the kernel shell
loop or a userspace `read`. A focus holder asleep in `InputWait` or `WaitAny`
polls nothing, so real keystrokes reached it only if something else happened
to be polling. The 19.5 and 19.6 gates published keys directly into WovenInput
and never exercised the hardware path. The handler now decodes and publishes
at IRQ time. Every lock on that path is an IRQ mutex at rank 10, and the two
`PreemptMutex`es an IRQ can interrupt are also rank 10, so neither inversion
nor recursion is possible. The polled fallback (`poll_legacy`) now leaves
auxiliary bytes for the mouse.

**Mouse.** `kernel/src/mouse.rs` drives the i8042 auxiliary port.
- Initialisation runs with interrupts off and polls with bounds: enable the
  port, set defaults, enable reporting (each acknowledged), then set the
  auxiliary IRQ bit and clear the auxiliary clock-disable bit in the
  controller configuration.
- IRQ12 is routed through the IOAPIC beside the keyboard. Unlike the keyboard,
  the mouse is optional: an unusable GSI leaves it unrouted rather than
  failing boot. Under the legacy PIC it is unmasked only if a mouse answered.
  Boot logs `[BOOT] ps2 mouse present=`.
- The handler takes only bytes flagged auxiliary and decodes three-byte
  packets: bit-3 resynchronisation, 9-bit signed deltas, Y flipped to screen
  orientation, overflow packets dropped whole, and button edges. It publishes
  `PointerMove` / `PointerButton`, which are now production WovenInput
  events rather than Stage 13.7 probe-only.

**Userspace.** `InputPoll` and `InputWait` encode pointer motion as kind 6
(two's-complement dx/dy halves) and buttons as kind 7. libwoven adds
`InputEvent` and `input_poll_event`; `input_poll` and `input_wait` keep
returning keys only, skipping pointer events. The window manager tracks the
pointer from relative motion; a left click on an uncovered part of the bottom
window raises and focuses it (click-to-focus).

Gate `[S19.12] pointer input: PASSED`. Every input travels the real i8042
path: command 0xD3 presents a byte as if the mouse sent it (IRQ12, the mouse
driver), and 0xD2 as if the keyboard did (IRQ1, the keyboard driver).
- The kernel checks the packet decoder, then the raw paths into a focus
  queue: a mouse packet arrives as motion plus press, then release; a
  keyboard make code arrives as a decoded key with no one polling.
- hello-gui `--wm` with two clients: the kernel moves the pointer into A
  only, clicks, and types `x`; then moves into B only, clicks, and types `y`.
- Each client must receive exactly its own key, and the kernel reads B on top
  at the overlap.

Negative control: with the IRQ-time decode removed, which was the pre-19.12
behaviour, the gate fails at "keyboard bytes through IRQ1 are decoded at IRQ
time". Evidence is in `audit-artifacts/stage19.12-negative-control-*`.

Open:
- PS/2 only. The USB HID mouse protocol is recognised by the xHCI driver but
  not yet decoded into pointer events.
- No scroll wheel (IntelliMouse) and no acceleration.
- The pointer position lives in the window manager. There is no cursor
  image: drawing one is the window manager's job, and there is no hardware
  cursor.

## Stage 19.11 — userspace window manager: stacking, raise, focus, input routing (2026-10-14)

**Roadmap mapping.** This repository's Stage 18–19 numbering has drifted
from `docs/master-development-roadmap.md`: the work from Stage 18.1 onward is
the roadmap's **Stage 17, WovenCompositor**. That stage lists windows,
surfaces, compositing, damage tracking, z-order, transparency, shadows,
animations, input routing, focus, multiple monitors and DPI scaling. Its
security rule is that applications must never directly control another
application's surface. The desktop shell is roadmap Stage 18 and follows it.
This increment covers z-order, focus and input routing.

**What it shows.** A complete userspace window manager runs on the existing
primitives, with no new kernel interface:
- the compositor lease and display (19.4, 19.3);
- input focus and `WaitAny` (19.5, 19.8);
- shared client surfaces (19.7);
- IPC.

hello-gui `--wm` takes the display, and with it focus. It receives each
client's shared-surface token, position and colour over IPC, and stacks the
windows, the older client at the bottom. It draws them bottom to top and, after
every redraw, checks its own screen: a pixel inside both windows must show the
top window, and an uncovered corner of the bottom window must show the bottom
window. Tab raises and focuses the other window. A typed character is forwarded
over IPC to the focused (topmost) window's client only. Clients never touch
each other's windows; the window manager reads them only through their grants.

Gate `[S19.11] window stacking and focus: PASSED`:
- `--wm` runs with `Console | DisplayIo | InputIo | Ipc`. Two ordinary
  `--wm-client` processes share overlapping 120×80 windows: A at (200,150),
  red; B at (260,190), blue.
- Whenever the window manager is asleep for input, the kernel publishes the
  next of Tab, `x`, Tab, `y` through the keyboard path.
- A must receive exactly `x` (it is on top after the first Tab), and B
  exactly `y` (on top again after the second Tab). Each client then waits a
  quiet period for stray keys. A misrouted key that arrives later still fails
  the gate: the window manager's send to an exited client fails, and the
  intended client never gets its key.
- The kernel reads the overlap (blue, B on top) and each window's private
  area from the compositor, and confirms lease and focus are released.

Negative control: with the window manager forwarding to the bottom window,
the wrong client receives the key (exit 156) and the gate fails. Evidence is
in `audit-artifacts/stage19.11-negative-control-*`.

The first run failed the window manager's own check (step 144): its "bottom
window still visible" probe used a corner the raised window covers in the
swapped layout. That was a test defect. The probe now picks an uncovered
corner. Evidence is in `audit-artifacts/stage19.11-probe-point-1cpu-*`.

Open (remaining roadmap Stage 17 items after the accepted Stage 19 gates):
- A window move or lower redraws all windows: the window manager tracks no
  exposed regions, and the kernel compositor has no z-order of its own,
  because stacking is policy, held in userspace.
- Multi-monitor output and per-display policy are not implemented.
- Scaling is integer-only and animation pacing is bounded software timing;
  fractional DPI and hardware GPU acceleration remain outside this gate.

Pointer-driven focus is closed by Stage 19.12: IRQ12 mouse packets and
IRQ1 keyboard input reach the event-driven window manager, clicks raise and
focus the selected window, and keys route only to that client. See the
[Stage 19.12 pointer audit](audit-stage19-12-pointer-2026-10-13.md).

## Stage 19.10 — compositor follows the framebuffer; one framebuffer writer (2026-10-12)

**Geometry.** The boot compositor is now sized to the framebuffer instead of
a fixed 640×480. `compositor::init_for_display` reads
`terminal::framebuffer_info` after the heap self-test and clamps to
1920×1200. A 4K display would otherwise cost a 33 MB kernel allocation; the
clamp bounds it near 9 MB, and a larger display shows the compositor in its
top-left corner. The self-tests that reset and re-initialise the compositor
still use the fixed default, so their expectations are unchanged.

**Single writer.** The early-boot `Console` no longer holds the framebuffer
slice. It keeps only the geometry, and each operation (character, clear,
scroll, backspace, desktop present) borrows the buffer once through
`terminal::with_text_framebuffer`. That accessor refuses while the
compositor owns the display. Every framebuffer write after `terminal::init`
now goes through the terminal lock: console, userspace terminal and
compositor. This closes the Stage 19.3 open item, which noted that the boot
console wrote outside the lock and had to move behind it before the
compositor could be activated at boot. `Console` lost its lifetime parameter;
its users in `shell.rs`, `smp.rs` and `physical_probe.rs` changed
accordingly.

Gate `[S19.10] display geometry and single writer: PASSED`:
- The boot compositor's dimensions equal the clamped framebuffer size
  (1280×800 on QEMU).
- Taking the display repaints exactly that many pixels, and the far corner of
  the framebuffer shows the compositor background, where before it was
  outside the compositor.
- Boot-console output leaves the framebuffer checksum unchanged while the
  compositor owns the display, and changes it after release.

Negative control: with `with_text_framebuffer` ignoring display ownership,
the gate fails at "boot console suppressed while compositing". The evidence
is labelled in `audit-artifacts/stage19.10-1cpu-1791825577739619200`.

Open:
- The production boot still does not activate the compositor. That is now a
  policy decision (when a desktop process starts) rather than a safety
  prerequisite.
- The kernel shell's desktop renderer keeps its own retained surface,
  presented through the same lock; it is not a compositor client.

Next prerequisite: a desktop/shell process that takes the lease at startup
and composites client windows, which is the Phase 5 desktop itself.

## Stage 19.9 — client damage notification and damage-only compositing (2026-10-12)

A client now tells the compositor what changed, and the compositor copies only
that.

- **Declaring damage:** syscall 112 (`GraphicsSurfaceDamage(handle, rect)`)
  lets the owner record a changed rectangle. The token is bound to the
  caller, and no capability is needed, since only the caller's own pixels are
  involved. Damage is clipped to the surface and unioned into one bounding
  rectangle per slot, so per-slot state stays fixed-size. It is cleared when
  the slot is freed. If the surface is shared, the grantee's damage waiter is
  woken after the registry lock is released.
- **Compositing damage:** syscall 113 (`GraphicsCompositorBlitDamage(token,
  origin)`) lets the lease holder take the pending damage of a surface shared
  with it, copy just that region, and blit it at its offset within a window
  placed at `origin`. It returns the region, or 0 when nothing was pending.
  The grant rules are the same as for the whole-surface shared blit.
- **Waiting on damage:** `WaitAny` gains source bit 2, damage on any surface
  shared with the caller, which requires `DisplayIo`. Waiters live in a
  bounded four-entry table in the registry. A grantee's exit drops its entry
  without waking, since that path holds the scheduler lock. The usual rules
  apply: one registration per grantee, live-holder refusal, and take-over
  from a dead holder.
- **libwoven:** `GraphicsSurface::damage`, `compositor_blit_damage ->
  Option<Region>`, and `wait::DAMAGE`.

Gate `[S19.9] damage compositing: PASSED`:
- The kernel first checks the damage rules: clipping, union, reads only
  through a grant, cleared on take, rectangles outside the surface ignored,
  damage dies with the slot.
- The probe is refused damage waits, damage on a foreign surface, and
  damage-only compositing.
- hello-gui `--damage-compositor` takes the lease and composites the client's
  declared full-window damage, (0,0,40,30); a second take finds nothing. It
  then sleeps in `WaitAny(DAMAGE)`.
- Once the kernel sees it `Blocked`, it tells `--damage-client` to go. The
  client recolours (10,5,4,3) and declares it, and recolours (30,20,2,2)
  without declaring it.
- The compositor must be woken before its 500-tick deadline and copy exactly
  (10,5,4,3). The screen must show the new colour there and the old colour
  where the undeclared change was. The kernel re-reads both pixels after
  both processes exit.

Negative controls (evidence in `audit-artifacts/stage19.9-negative-control*`):
- With the wakeup removed from `mark_damage`, the first version of the gate
  still failed, but only through the kernel's wait for the client timing out.
  The compositor itself passed: `WaitAny` is level-checked, so at its
  deadline it correctly reported damage that had arrived without a wakeup.
- The compositor now times its wait and fails if only the deadline woke it.
  The same control then fails crisply at that step (exit 111).

Open:
- One bounding rectangle per surface: two small distant changes copy the
  span between them.
- Damage is not tied to a particular blit position; the compositor supplies
  the window origin each time.
- No per-client damage queue: `WaitAny` says some shared surface has damage,
  and the compositor asks each of its clients.

Next prerequisites: the compositor surface following real framebuffer
geometry instead of a fixed 640×480, and boot-time activation once the early
boot console writes through the terminal lock.

## Stage 19.8 — one sleep over input and IPC (2026-10-12)

A process can now sleep until any of several sources is ready. Syscall 111
(`WaitAny(deadline, mask)`) takes a mask: bit 0 is input (`InputIo`, live only
while the caller holds focus), bit 1 is the caller's pid mailbox (`Ipc`). It
returns the mask of ready sources, or 0 when the timer reaches `deadline`.
Nothing is consumed: the caller follows up with `InputPoll` or
`MessageReceive`. Empty or unknown masks are refused, and so is waiting on
input alone without focus, which could never complete.

The pid mailbox gained a waiter. `LegacyEndpoint` holds one optional `TaskId`;
`ipc::send` takes it under the IPC lock and calls `signal_event` after
releasing it. Endpoint teardown drops the waiter without waking, as focus
withdrawal does. `woven_input` gained `register_waiter`, `clear_waiter` and
`pending`, so the same focus waiter slot serves both `InputWait` and
`WaitAny`.

The wait registers with every requested source, then re-checks readiness, then
sleeps. An event between registration and sleep latches the scheduler's
pending permit, so it cannot be lost. A slot held by another live task of the
same process is refused; a slot held by a dead task is taken over, with
liveness checked outside the source's lock. All registrations are dropped
before returning.

Gate `[S19.8] combined wait: PASSED`:
- An unprivileged probe is refused `WaitAny` on input, and refused empty and
  unknown masks.
- hello-gui `--multiplex <peer>` takes focus and sleeps over both sources.
- The kernel registers a stand-in peer mailbox. Once it sees the client
  `Blocked`, it sends a message; the client must report exactly `MESSAGE` and
  receive `ping` from the peer.
- Once the client is `Blocked` again, the kernel publishes a key; the client
  must report exactly `INPUT` and read `z`.
- A 20-tick timed wait then returns 0, no earlier than its deadline, and focus
  returns on exit.

Negative control: with the wakeup removed from `ipc::send`, the client sleeps
through the message and wakes on the key with both sources ready (exit 83), so
the gate fails. Evidence is in `audit-artifacts/stage19.8-negative-control-*`.

Two problems found while bringing this up:
- The first design had the client reply to the stand-in peer. `may_ipc_with`
  refused it, correctly: the peer is a bare mailbox with no process
  credentials. The reply step was removed instead of relaxing the policy.
  Evidence is in `audit-artifacts/stage19.8-reply-refused-1cpu-*`.
- **Harness defect.** The three app harnesses built the app with
  `cargo build`, inheriting `CARGO_TARGET_DIR` from the caller's shell. With
  the project's documented environment, the fresh binary went to the kernel's
  target tree, while the kernel embedded the stale binary in
  `apps/*/target`. Earlier gates were unaffected only because the app had
  been built by hand from the same source before each run. All three
  harnesses now build with the app's own target directory pinned and the
  kernel's build-directory override removed.

Open:
- The pid mailbox has one waiter; handle-based endpoints keep their existing
  multi-waiter queues and are not yet `WaitAny` sources.
- Timers and event objects are not `WaitAny` sources either; the deadline
  covers the common timeout case.

Next prerequisite: client-to-compositor damage notification, so a compositor
sleeping in `WaitAny` redraws only what a client changed.

## Stage 19.7 — client surfaces delegated to a compositor process (2026-10-12)

A separate compositor process can now draw other clients' windows, with each
client's consent.

**Sharing:** a client grants one process read access to one of its own
surfaces with syscall 109 (`GraphicsSurfaceShare(handle, grantee)`; grantee 0
revokes). Sharing needs no capability: a process may always decide who reads
its pixels. The token still goes through `caller_surface`, so only the owner
can share. Each surface carries at most one grant, in the slot. The grant is
cleared whenever the slot is freed, by destroy or by the owner's exit, so it
never outlives the surface or carries over to the slot's next occupant. Pids
are never reused, so a grant naming an exited process is inert.

**Compositing:** the lease holder draws a shared surface with syscall 110
(`GraphicsCompositorBlitShared(token, x | y << 32)`). It requires `DisplayIo`
and the lease. The token is the owner's, passed out of band, so it is
deliberately not bound to the caller. Instead
`user_graphics::snapshot_shared` requires slot, generation and owner to match
and the slot's grant to name the caller: a token alone confers nothing. The
snapshot-then-blit lock pattern is the same as `blit_for`.

**libwoven:** `GraphicsSurface::share_with` / `unshare`,
`compositor_blit_shared`, and typed IPC: `message_send` /
`message_receive` over the existing syscalls 11 and 12.

Gate `[S19.7] surface delegation: PASSED`:
- The kernel first checks the grant rules: nothing is shared by default; the
  grantee is allowed while a stranger and the owner itself are not; stale and
  forged tokens are refused even for the grantee; revocation works; a grant
  dies with its surface.
- hello-gui `--compositor` (`Console | DisplayIo | Ipc`) takes the lease.
  hello-gui `--client <pid>` runs as an ordinary process: it shares one
  surface with the compositor, keeps a second private, checks it cannot blit
  anything itself, and sends both tokens over IPC.
- The compositor must draw the shared surface and see the client's colour on
  the screen. It must be refused the private surface even with its genuine
  token, and refused the shared one through the owner-only blit path. It then
  presents and replies; the client waits for that reply so its surfaces stay
  alive until drawn.
- After both exit, the kernel reads the client's colour at two corners of the
  composited window, and checks that the client's surfaces and grants and the
  lease are gone.

Negative control: with the grant check bypassed in `snapshot_shared`, the gate
fails at the grant-rules check. Evidence is in
`audit-artifacts/stage19.7-negative-control-*`.

Open:
- A shared surface is copied whole at each blit, and there is no damage
  notification from client to compositor: the compositor redraws when it
  chooses.
- One grantee per surface.
- Token transfer is by plain IPC bytes. It cannot be forged into access,
  because the grant is the authority, but it is not a kernel-transferred
  handle.
- IPC receive is still non-blocking, so both test processes poll with a
  bounded deadline.

Next prerequisites: a blocking IPC receive (or a combined wait with input),
and client-to-compositor damage notification.

## Stage 19.6 — event-driven input wait (2026-10-12)

The focus holder can now sleep until input arrives instead of polling.
Syscall 108 (`InputWait(deadline)`) returns the next event, or `WOULD_BLOCK`
when the timer reaches `deadline` (`u64::MAX` waits indefinitely). It has the
same authority as `InputPoll`: `InputIo` plus focus. libwoven wraps it as
`input_wait(Option<Ticks>)`.

`woven_input::wait_for` registers the calling task as the focus waiter under
`FOCUS`, releases the lock, and sleeps with
`task::wait_for_event_until(deadline)`. `publish` takes the waiter out under
`FOCUS` and calls `task::signal_event` after releasing it. No wakeup is lost:
a publish between registration and sleep latches the scheduler's pending
permit, and the sleep consumes it. Unrelated wakeups simply re-check.

Lock order:
- The task-exit path takes the scheduler lock, then the compositor lock, then
  `FOCUS`. So `FOCUS` is never held while the scheduler lock is taken: waking
  and liveness checks happen with it released.
- Focus withdrawal forgets the waiter rather than waking it. It runs under the
  scheduler lock on the exit path, and only the owner's exit withdraws focus,
  which takes every task that could be waiting with it.

Waiter rules:
- One task per process may wait. A second concurrent waiter is refused, unless
  the registered one has died, in which case `task::task_live` (checked after
  releasing `FOCUS`) lets the newcomer take over by compare-and-set. Task ids
  are never reused, so a dead answer is final.
- A timed-out wait unregisters itself and returns any event that raced in
  after its last check.

Gate `[S19.6] blocking input wait: PASSED`:
- An unprivileged probe must be refused `InputWait`, with a bounded deadline
  so a broken check cannot hang the gate.
- hello-gui `--wait` gains focus and reads `o`, `k`, Enter through
  `input_wait(None)`. Before publishing each key, the kernel waits until the
  client's task is `Blocked`. A polling client is only ever `Ready`, so this
  proves it sleeps in the kernel and is woken by the publish itself.
- The app then makes a 20-tick timed wait with nothing pending. It must return
  `None`, and no earlier than the deadline.
- Focus returns on exit.

Negative control: with the sleep in `wait_for` replaced by `yield_now` (a
spinning wait), the gate fails at "client sleeps while waiting". Evidence is in
`audit-artifacts/stage19.6-negative-control-*`.

Open: the waiter slot is per process, not per thread, so a multi-threaded
client must funnel input through one thread. There is no combined wait on
input plus other events: `InputWait` is separate from the event objects, so a
client cannot yet sleep on "input or timer or IPC" in one call. Next
prerequisite: delegating client surfaces to the holder by capability, so a
separate compositor process can draw other clients' windows.

## Stage 19.5 — input focus follows the compositor lease holder (2026-10-12)

When the compositor lease holder takes the display (its first present), it
also takes input focus. From then on, `woven_input::publish` delivers every
event into a bounded 128-entry focus queue that only that process can read.
The terminal's queue gets nothing, because the user is looking at the
holder's screen, not the hidden terminal. When the holder exits,
`compositor::release_owner` withdraws focus and the terminal receives input
again. Every focus change discards the focus queue, so keys typed for one
owner never reach the next owner or leak to the shell. `compositor::reset`
also releases a holder's focus.

The focus state is an IRQ mutex, because the keyboard interrupt path
publishes through it. Lock order: compositor or keyboard decoder (rank 10),
then `FOCUS`, then the terminal `QUEUE`. `FOCUS` is held across the fallback
push so focus cannot change between the check and the push.

Syscall 107 (`InputPoll`) returns the next event as `kind << 56 | value`
(character, Enter, Backspace, Tab, F1), `WOULD_BLOCK` when the queue is empty,
or `ERROR`. It requires `InputIo`, and the caller must hold focus. libwoven
wraps it as `input_poll() -> Result<Option<Key>, Error>`. Pointer events exist
only in the Stage 13.7 probe build and have no userspace encoding yet.

Gate `[S19.5] input focus: PASSED`:
- The kernel first checks the focus rules: no focus routes to the terminal;
  focus routes only to the owner; non-owners are refused; the queue is bounded
  and overflow is refused; a release by a non-owner is ignored; a focus change
  discards pending events.
- An ordinary-process probe must be refused `InputPoll`, along with all the
  Stage 19.4 refusals, against a genuinely foreign surface.
- hello-gui `--input` runs with `Console | DisplayIo | InputIo`. Once it holds
  focus, the kernel publishes `o`, `k`, Enter through the keyboard interrupt's
  own path. The terminal queue must stay empty, and the app must receive
  exactly those keys in order with nothing extra. After exit, focus is gone
  and a keystroke reaches the terminal again.

Negative control: with `publish` changed to ignore focus, the gate fails at
the focus-rules check. Evidence is in
`audit-artifacts/stage19.5-negative-control-*`.

Two defects were found while bringing this up:
- The first `publish` returned `QUEUE.lock().push(event)` as its tail
  expression. Rust drops tail-expression temporaries after locals, so the
  `FOCUS` guard was released before `QUEUE`, out of nesting order. The
  lock-order tracker panicked on the first unfocused keystroke, which the boot
  sequence produces in every build. The push result is now bound and `FOCUS`
  dropped explicitly.
- `kernel_panic` parked forever, so under `qemu-test` that panic surfaced as a
  300-second host timeout. It now exits QEMU with the failure status after
  printing the panic report, as `halt()` already did. Every harness already
  treats `KERNEL PANIC` as a failure. The timeout evidence is in
  `audit-artifacts/stage19.5-timeout-1cpu-*`.

Open: `InputPoll` is non-blocking, so clients poll with `yield_now`. A
blocking wait belongs on the existing event objects, so a client sleeps until
input arrives. Focus is all-or-nothing per holder, with no per-window focus
yet; that is the holder's job once it composites several clients. Next
prerequisites: an event-driven input wait, then delegating client surfaces to
the holder by capability.

## Stage 19.4 — compositor authority and client handle binding (2026-10-12)

**Defect fixed from Stage 18.** The client surface registry compared a slot's
owner with the owner field inside the handle, but userspace supplies that
handle. Any process could forge a token naming another process as owner, and
the slot index (0–15), generation (low after boot) and pid are all small and
guessable. With that token it could read, fill, stroke, destroy or blit the
other process's surface. The Stage 18.1 impostor test only swapped the owner
field of a genuine token, so it never exercised a forgery. Every graphics
syscall now resolves its token through `caller_surface`, which requires the
token to name the calling process; the registry then checks slot, generation
and owner as before.

**Compositor authority.** Blitting onto the compositor, reading screen pixels
and presenting to the framebuffer now require the `DisplayIo` capability. They
also require the compositor lease, which exactly one process holds at a time:
`compositor::claim` (syscall 105) takes it, and claiming again as the holder
succeeds. Reading the screen carries the same authority as drawing on it,
because the screen holds every client's pixels. Syscall 106 presents
compositor damage to the framebuffer, taking the display from the terminal on
first use. The task-exit path calls `compositor::release_owner`, which drops
the lease and gives the display back, so a crashed holder cannot keep the
screen. Ordinary user processes do not carry `DisplayIo`; client surfaces
(syscalls 98–101, 104) still need no capability, since a process can only
reach its own.

**libwoven.** `libwoven::entry!(main)` defines `_start` as a three-instruction
assembly stub that passes the entry stack pointer to `main(Args)`. Rust code
cannot observe that pointer reliably, which is the only reason the stub is
assembly. `Args::get` bounds each string scan by the kernel's 256-byte argument
limit. Also new: `compositor_claim()` and `compositor_present()`.

**Gate.** `[S19.4] compositor authority: PASSED`. The kernel first checks the
lease rules directly: exclusivity, idempotent re-claim, non-holders refused
blit, read and present, and a release by a non-holder ignored. It then creates
a surface owned by another pid and runs hello-gui twice:
- `--probe <token>` as an ordinary user process. Claim, blit, screen read and
  present must all fail. Every operation on the foreign surface must fail,
  both with its genuine token and with a token forged to name the probe. The
  kernel then checks that the foreign surface still holds its colour and that
  the probe never held the lease.
- Normal mode with `Console | DisplayIo`. The app claims, blits, verifies the
  screen and presents. The kernel then reads the framebuffer itself and finds
  the window's content colour, and checks that the lease was released, the
  display returned to the terminal, and the app's surfaces were reclaimed.

A negative control was run: with `caller_surface` removed from the pixel
syscall alone, the probe's forged read succeeds, it exits 207 and the gate
fails. The evidence is in `audit-artifacts/stage19.4-negative-control-*`.

Stages 18.2 and 19.2 now start hello-gui through the system-service spawn path
with `Console | DisplayIo`, because the app needs display authority. Their
assertions are unchanged.

Open: `DisplayIo` is granted at spawn by the kernel; there is no service
manager or user-facing grant flow. The lease has no revocation other than the
holder's exit, and no hand-over to a successor compositor. The holder still
blits clients' surfaces itself, so there is no way yet for one process to
composite another's surface by capability. Next prerequisites: input events
routed to the lease holder, then delegating client surfaces to the holder by
capability so a separate compositor process can draw them.

## Stage 19.3 — compositor presentation and display ownership (2026-10-12)

The compositor surface now reaches the real framebuffer.
`compositor::activate_display()` takes the display from the text terminal and
marks the whole surface damaged. `compositor::present()` copies only the damaged
regions through `display::present` and clears the damage after a successful
copy, so a failed presentation loses nothing. `deactivate_display()` returns the
display to the terminal.

There is exactly one path to the framebuffer after boot.
`terminal::with_framebuffer` lends the buffer under the terminal's lock, so the
compositor keeps no pointer of its own and its writes cannot interleave with
terminal text. While the compositor owns the display, terminal rendering is
suppressed; when it releases the display, terminal rendering resumes. The
compositor lock changed from an IRQ mutex to a preemption mutex: copying a
1280×800 frame with interrupts disabled would stall device interrupts, and
nothing takes the lock from interrupt context. Lock order is compositor, then
terminal, both at rank 10. The terminal never calls the compositor.
`with_framebuffer` deliberately avoids `file_fault_io`, which takes the
scheduler lock: the framebuffer mapping is resident and cannot fault.

The self-test reads framebuffer bytes back itself, converting by the reported
pixel format, rather than trusting the compositor's state. It checks:
presentation is refused before takeover; takeover repaints exactly the visible
area (640×480 on QEMU's 1280×800 framebuffer); the screen shows the compositor's
black background where the terminal's background was dark blue; a second
present writes nothing; one 32×16 client blit presents exactly 512 pixels, with
the client colour at its offset and background next to it; terminal output
leaves the framebuffer checksum unchanged while the compositor owns the display;
presentation is refused after release; and terminal output changes the
framebuffer again afterwards. Marker: `[S19.3] compositor presentation: PASSED`.
Passed on 1, 2 and 4 CPUs.

Open: nothing in the production boot activates the compositor yet. The early
boot `Console` still writes the framebuffer directly, outside the terminal lock.
That is safe only while it runs single-threaded before the scheduler starts, and
it must move behind `with_framebuffer` before the compositor is activated at
boot. Userspace cannot present: activation and presentation are kernel calls,
pending a compositor capability. Terminal output while compositing is not shown
anywhere on screen; a terminal client of the compositor would replace that.
Next prerequisites: the compositor capability (a single holder may present),
then input events routed to the focused client.

## Stages 18–19 — client surfaces and the compositor primitive (2026-10-12)

A userspace process can now own a drawing surface, draw into it through
syscalls, and place it on a kernel-held compositor surface that outlives the
process.

**Stage 18.1 — client surface registry.** `kernel/src/user_graphics.rs` is a
bounded, owner-tagged registry of `display::Surface` objects: 16 surfaces
system-wide, 4 per process, at most 320×240 each. A handle packs slot (8 bits),
generation (24) and owner pid (32). Every operation checks all three, so a stale
handle or another process's handle is refused rather than resolving to a reused
slot. `release_owner(pid)` runs on the task-exit path in `task.rs`, next to the
other per-process registries. Syscalls 98–101 cover create, fill, pixel
readback and destroy, and 104 covers stroke. Fill and stroke pack x/y/w/h as four
u16 values in one register, because the ABI carries three arguments. A text
syscall was tried and removed: handle, origin, pointer, length and colour do not
fit in three arguments. Text drawing stays kernel-internal until the syscall ABI
gains an argument block. Marker: `[S18.1] client surface registry: PASSED`.

**Stage 18.2 — end to end through a real application.** `apps/hello-gui`
builds outside the kernel, like `apps/hello`. It allocates a surface, draws a
window (background, title bar, content, one-pixel border) and reads back seven
pixels, one in each region and edge, through the pixel syscall. Each mismatch
exits with its own status. The kernel then checks the app's exit status and that
`count_for(pid) == 0` after exit. Marker:
`[S18.2] client surface end-to-end: PASSED`.

**Stage 19.1 — compositor surface.** `kernel/src/compositor.rs` holds one
640×480 `Surface` plus a `Damage` list. It is initialised lazily after the heap
self-test, because a 1.2 MB allocation earlier in boot fails. `blit` takes a
snapshot of the client surface under the `user_graphics` lock, releases that
lock, and then takes the compositor's. Both locks are rank 10, so the two
subsystems never nest and need no ordering rule between them. The self-test
covers: refusal before init (NotReady regardless of the handle), idempotent
init, exact colour at the blitted offset and untouched pixels next to it,
edge-clipped touched rectangles, refusal of a bogus handle, and refusal of a
handle whose owner has exited. Syscalls 102 (blit, `x | y << 32` as i32) and
103 (screen pixel). Marker: `[S19.1] compositor surface: PASSED`.

**Stage 19.2 — application through the compositor.** `hello-gui` presents its
window at (60, 40), reads screen pixels back from userspace, and exits. The
kernel then reads `compositor::pixel(160, 100)` itself, after the process has
gone. A userspace check that only claimed success cannot pass this gate, and the
read also shows that composed pixels do not depend on the client's lifetime.
Marker: `[S19.2] hello-gui through the compositor: PASSED`.

Validation: Stages 17.1, 18.1, 18.2, 19.1 and 19.2 passed on 1, 2 and 4 CPUs.
The kernel Clippy matrix (49 feature configurations) is clean with
`-D warnings`, as are libwoven and host Clippy (`--all-targets`). The first
Stage 19.1 run failed on all CPU counts because `blit` snapshotted the client
before checking readiness. It returned `InvalidClient` where `NotReady` was
specified, and the fix checks readiness first.

Historical note: the following paragraph describes the pre-19.3 baseline; the
later Stage 19 gates supersede those limitations.

Open: the compositor surface is not yet presented to the real framebuffer. It is
a fixed 640×480 off-screen surface, because no global framebuffer accessor
exists for it. There is no stacking, focus or input routing: a blit lands where
the client asks, so this is the primitive a userspace window manager will drive,
not a window manager. Any process may blit its own surfaces anywhere. Before a
second graphical client is admitted, that authority must become a capability.
Blits copy the whole client surface (at most 300 KB) per call. Text drawing
from userspace needs the argument-block ABI. Next prerequisites: framebuffer
presentation of compositor damage, a compositor capability, and keyboard and
pointer events delivered to the focused client.

## Stage 17.1 — WovenHat font and software compositing (2026-10-12)

`kernel/src/font.rs` covers all 95 printable ASCII glyphs at 5×7. The old table
had 14, so "WOVENHAT DESKTOP" could not render. `kernel/src/display.rs` adds
`Surface`, bounded `Damage` (16 rectangles, then a bounding box) and a single
`present` path that resolves the pixel format once per run. Host tests in
`tests/display_compositing.rs` cover clipping, stride and damage coalescing.
Marker: `[S17.1] WovenHat font coverage: PASSED`. Passed on 1, 2 and
4 CPUs in the Stage 18–19 validation.

## Stage 16.1 — applications built outside the kernel (2026-10-14)

WovenHat runs a program produced by the ordinary Rust toolchain. Before this,
every userspace program was one of 32 `global_asm!` blocks in
`kernel/src/userspace.rs`, wrapped in a synthetic ELF by `build_stub_elf` at
runtime, so a program could not exist without editing the kernel. That was the
binding constraint on a userspace platform, and therefore on any desktop.

`apps/hello` is a separate crate, excluded from the kernel workspace, with its
own linker script and link flags. It compiles to a static ET_EXEC image; the
dedicated harness signs that image into a WAPP package before the kernel build.
The kernel verifies the package, installs it into the filesystem, reads it back,
and loads the verified payload through the same ELF loader every other
executable uses. The self-test loads the bytes read from the filesystem rather
than the embedded constant, so the gate proves signature enforcement and that
path rather than assuming either.

`libwoven` grew from two bound syscalls to the process, I/O, filesystem, time
and environment core, in two layers: one `unsafe` wrapper per kernel entry point
returning the kernel's value unchanged, and a safe surface returning
`Result<_, libwoven::Error>` where the unsafety can be discharged. The kernel
implements 98 syscalls; the remainder are reachable through
`libwoven::syscall::invoke` and should grow typed wrappers as they are used.

Three build settings follow from the loader's rules and the user address space,
each documented in [the application guide](writing-wovenhat-applications.md):
`code-model=large`, because `USER_REGION_START` is far outside the small code
model's ±2 GB; `build-std = ["core"]`, because the precompiled `core` uses the
small model and conflicts; and an explicit `PHDRS` linker script with
`-z norelro`, because the loader rejects `PT_GNU_RELRO` and any
writable-executable segment.

The marker is `[S16.1] out-of-kernel application: PASSED`, with per-step markers
for WAPP verification, tamper refusal, unsigned-external-image refusal, loader
acceptance, filesystem install, read-back, load, spawn, completion and exit
status. The dedicated application harness now passes on 1, 2 and 4 CPUs; see
the [Stage 16.1 audit](audit-stage16-1-out-of-kernel-application-2026-10-14.md).

The original Stage 16.1 boundary remains intentionally single-package and
VFS-backed; the release-level Stage 19 catalog/production-image gates now cover
the ten-package install plan and are recorded above. Shared libraries and
stack-size negotiation remain outside the bounded loader contract; static
initial-exec TLS and self-contained dynamic relocation are now supported.

The accepted Stage 19 applications now have a reproducible packaging boundary:
`scripts/package-system-apps.py` builds all ten applications, signs each WAPP
payload, and emits hashed `/bin` install paths plus `/share/applications`
descriptors in an install-plan manifest. The release image consumes that plan
and installs all ten packages transactionally in RAM after verification.

The feature-gated multi-application catalog gate now passes on 1/2/4 CPUs.
It consumes all ten signed packages, rejects tampered copies, verifies VFS
byte-for-byte read-back and normal ELF loading, and publishes matching catalog
descriptors. The normal-boot production image gate confirms installation
before shell startup. See the
[Stage 19 system-app catalog audit](audit-stage19-system-app-catalog-2026-10-14.md).
Persistent-media installation, power-loss rollback, and deployment-key
rotation/revocation remain separate production requirements.

## Stage 15 — WovenFS native filesystem (2026-10-12)

WovenFS is WovenHat's own on-disk filesystem, specified in
[the format document](wovenfs-on-disk-format.md). Its design constraint is that
no operation depends on more than one sector being written atomically: the
superblock alternates between two checkpoint slots rather than being overwritten
in place, and every multi-sector change is staged in a write-ahead journal whose
completion is decided by a single commit sector.

Stage 15.1 covers the filesystem itself — format, mount, create, write, read,
unlink, lookup, read_dir and a consistency check, with checksummed inodes and
file contents. Crash recovery is proven rather than asserted:
`tests/wovenfs_crash.rs` counts the writes each of format, create, write and
unlink performs, replays the operation once per write boundary against a fresh
image with the device refusing every later write, and at each point requires the
volume to mount, the consistency check to pass, and the operation to be entirely
absent or entirely present. Replay idempotence, checkpoint-slot fallback and
torn journal payloads are covered too.

Stage 15.2 covers WovenFS on a real block device through the ATA driver, with
its root published into the VFS at `/wfs`. It passed on 1/2/4 CPUs against a
scratch image and on an 8 GB USB device, verified independently from the host:
LBA 0 reads `WOVENFS1` with the checkpoint sequence in the slot the alternation
scheme predicts.

Physical qualification exposed an ATA defect invisible to emulation. The drive
wait was bounded by iteration count rather than time, and real flash stalls for
tens of milliseconds once its write buffer fills, so the driver read a normal
programming cycle as a fault and format failed at the 320th sector write. The
wait is now wall-clock bounded. This affected every ATA read and write in the
kernel, not only WovenFS.

Open: WovenFS is not the mounted system filesystem — FAT32 still holds `/mnt`
and the boot volume — and whether to make it one or to keep FAT32 and reclassify
Stage 12.2/12.4 is an open product decision. The bounded `/wfs/<name>` file
authority now supports durable create, write, read-back, and unlink through task
file syscalls; direct generic VFS mutation remains read-only. Native mutations
serialize their complete read-modify-write transactions. The adapter remains a
Nested native paths, directory creation, empty-directory removal, and rename
are now transactional; directory rename rejects descendant cycles with a
bounded fail-closed walk. Recursive import into the VFS mount and mounted-system
policy remain open. See the [Stage 15
VFS write-through audit](audit-stage15-wfs-vfs-write-through-2026-10-14.md).

Crash recovery is proven by fault injection, not actual power loss on a device.
Physical qualification remains incomplete: the inserted USB flash disk was
inspected read-only, while the destructive harness requires Administrator
elevation and explicit authorization to erase it.

The complete Stage 12 acceptance chain was revalidated on 2026-10-13: all
Stages 12.1–12.5 passed on 1/2/4 CPUs with exit 33. See the
[revalidation audit](audit-stage12-acceptance-2026-10-13.md). This confirms the
bounded gates only; the integration and physical limits above remain open.

## Stage 15.3 — runtime permission broker (2026-10-13)

The bounded permission broker now exposes generation-safe request/poll/decide/
revoke operations through the userspace ABI. Decisions require `TaskControl`
and successful changes pass through WovenGuard grant/revoke lineage checks;
security-ledger events record each request and decision. The 1/2/4-CPU gate,
freestanding Clippy, and all-target Clippy pass. See the
[Stage 15.3 permission-broker audit](audit-stage15-3-permission-broker-2026-10-13.md).
Graphical prompts, signed application identity, persistent consent, secure or
measured boot, and WovenVault remain open Stage 15 work.

## Stage 15.4 — signed application cryptographic boundary (2026-10-13)

The pure-Rust `no_std` Ed25519 verifier and bounded versioned WAPP envelope
now reject malformed and tampered payloads. The exact 1/2/4-CPU gate and
warning-denying freestanding Clippy pass; see the
[Stage 15.4 audit](audit-stage15-4-signed-applications-2026-10-13.md).
Release builds now require `WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX`; the RFC
self-test remains pinned to the development vector. Signed package
installation now has a real external-application path through
`tools/wapp-packager`, and Stage 16.1 verifies that package before and after its
VFS round trip. Multi-application catalogs and rollback-safe RAM publication
are now covered by the ten-app release gates. Deployment-key
rotation/revocation and firmware/installer-owned key slots remain open. User
`exec`/`spawn` now reject raw external ELF; only the finite kernel-owned system
image allowlist retains the explicit internal-image path.
Stage 15.5 software signed-kernel handoff is accepted with the exact 1/2/4-CPU
gate; see the [secure-boot audit](audit-stage15-5-secure-boot-2026-10-13.md).
Firmware UEFI Secure Boot state, TPM/event logs, release-key provisioning,
rollback/revocation, and signed-initrd enforcement remain hardware/platform
work and are not claimed complete.

## Stage 15.7 — WovenVault volatile secret handles (2026-10-13)

WovenVault now provides generation-safe per-owner secret handles with bounded
storage, validated copy-in/copy-out, zeroizing revoke, and owner teardown.
The `Secrets` capability is not granted by default. The 1/2/4-CPU gate and
warning-denying freestanding kernel Clippy pass. See the
[Stage 15.7 WovenVault audit](audit-stage15-7-wovenvault-handles-2026-10-13.md).
The Stage 15.6 software measurement audit records the separate 1/2/4-CPU
kernel measurement gate; TPM PCR extension, event logs, and firmware Secure
Boot state remain hardware-dependent.
Persistent encrypted records and crash-safe successor rotation now pass at the
kernel boundary; TPM/firmware provisioning, deployment-key custody, and a
secrets UI remain open.

## Stage 15.8 — security-ledger read ABI (2026-10-13)

The security ledger now has a bounded newest-relative read syscall authorized
by `TaskInspect`, with validated userspace copies and a fixed 48-byte record
decoded by typed `libwoven` wrappers. The 1/2/4-CPU gate, build, and
warning-denying freestanding kernel Clippy pass. See the
[Stage 15.8 security-ledger audit](audit-stage15-8-security-ledger-2026-10-13.md).
This is an ABI boundary, not a complete security-center product: durable
retention/export, signed identity, graphical review, and secure/measured boot
remain open.

## Stage 15.9 — exploit-mitigation hardening (2026-10-13)

The existing ASLR, W^X, and user-stack guard-page checks are now joined by a
scheduler-validated canary on each static kernel task stack. The dedicated
1/2/4-CPU gate and warning-denying freestanding kernel Clippy pass. See the
[Stage 15.9 exploit-mitigation audit](audit-stage15-9-exploit-mitigations-2026-10-13.md).
This closes the verified software-hardening subset only: true unmapped kernel
stack guards, SMEP/SMAP enablement and hardware qualification, shared-library
RELRO/TLS support, and scheduler-backed user threads remain open.

## Stage 1–5 bounded batch data journal — 2026-10-06

FAT32 persistence now exposes a separate versioned `WDJ2` journal for bounded
sixteen-file data batches. Prepared batches roll every partially-written file
back to its prior bytes and metadata, removing stale WMD1/WMD2 metadata
intents; committed batches are retired only after all intended checksums and
metadata snapshots match. The legacy `WDJ1`
single-file format and metadata ABIs remain readable. Kernel
build/check, warning-denying kernel Clippy, and the existing storage gate
passed on 1/2/4 CPUs, including two-file commit/readback and prepared,
partial-commit, and complete-commit recovery exercises. See the
[batch-journal audit](audit-stage1-5-batch-journal-2026-10-06.md).

Physical storage/DMA qualification, unclean-shutdown hardware testing,
metadata transactions, and arbitrary-size transactions remain open.

## Stage 1–5 immediate batch rollback — 2026-10-13

The bounded WDJ2 data-batch API now invokes durable recovery immediately when a
post-journal write, metadata, commit-record, or journal-retirement operation
fails. It invalidates file-page cache state before recovery, preventing a
transient runtime error from leaving partially committed data on disk or stale
pages available to the current boot's VFS; the original error is preserved and
a failed rollback remains recoverable from the durable journal. Direct FAT32
and MBR/GPT-partitioned paths are covered. The 1/2/4-CPU disposable storage
gates, host crash tests, and warning-denying Clippy passed. See the
[immediate-rollback audit](audit-stage1-5-immediate-rollback-2026-10-13.md).

This is a bounded hardening increment, not full Stage 1–5 completion. The
follow-up metadata transaction increment below closes metadata rollback within
this bounded format; arbitrary-size/full-volume rollback, physical storage/DMA
qualification, and unclean-shutdown hardware evidence remain open.

The WDJ2 capacity extension was revalidated on 1/2/4 CPUs after increasing the
bounded batch maximum from four to sixteen entries; see the [capacity audit](audit-stage1-5-wdj2-capacity-2026-10-14.md).

## Stage 1–5 bounded metadata transactions — 2026-10-14

WDJ2 v2 snapshots old and new inode/sidecar ownership metadata while retaining
version 1 recovery compatibility. Prepared and incomplete committed batches
restore both bytes and prior metadata; committed recovery retires the journal
only after bytes and new metadata match. The live FAT32 recovery fixture and
storage/SMP QEMU gates passed on 1/2/4 CPUs. See the [metadata transaction
audit](audit-stage1-5-metadata-transactions-2026-10-14.md).

This closes bounded metadata transactions, not arbitrary-size/full-volume
rollback or physical unclean-shutdown qualification.

## Stage 1–5 sector-write fault injection — 2026-10-14

FAT32 mutation self-tests now inject a deterministic `DeviceFault` at every
bounded file-creation sector-write boundary. Failed mutations must leave the
volume structurally checkable, remove the new file, and preserve the original
file bytes; successful mutations are also checked. Host and freestanding
storage gates use the same self-test. This is software error-return evidence,
not physical power-loss or torn-sector evidence. FSInfo remains an advisory
hint and may be stale after an injected failure; physical flash/ATA/DMA,
torn-write, arbitrary-size/full-volume rollback, and reboot recovery remain
open.

The WDJ2 codec self-test also rejects every byte-truncated form of a valid
prepared record before recovery can interpret it. This strengthens software
journal-boundary evidence; it does not prove physical torn-sector atomicity or
power-loss recovery.

## Stage 1–5 DMA allocation cleanup — 2026-10-14

The existing AHCI and NVMe DMA-page owners now return allocator frames when
direct-map setup fails. AHCI also drops all pages on failed controller
initialization; successful controller ownership remains process-lifetime and
is held under the controller lock until no hardware command can reference the
pages. The `BlockDevice` ABI is unchanged.

The Stage 13.3/13.4 freestanding Clippy gates, host Clippy, and the complete
host test suite passed after this change. The NVMe and AHCI QEMU controller
gates also passed on 1/2/4 CPUs with exit 33. See the [DMA allocation audit](audit-stage1-5-dma-allocation-2026-10-14.md)
for the six evidence directories.
This closes a software allocation-leak path only. Physical AHCI/NVMe and USB
flash qualification, IOMMU isolation, interrupt completion, torn-sector and
power-loss evidence, and arbitrary-size rollback remain open.

## Stage 14.2D — Router Advertisement state (2026-10-06)

Router Advertisement state now tracks a link-local router, bounded lifetimes,
and one validated prefix with expiration and withdrawal. It rejects invalid
preferred/valid lifetimes and global router sources. Check the remote D audit
for its exact acceptance evidence.

## Stage 14.2E — ICMPv6 checksum boundary (2026-10-06)

The IPv6 foundation now has production-reusable address and prefix primitives:
prefix containment, link-local/multicast classification, and RFC 4291
solicited-node multicast derivation. The 1/2/4-CPU Stage 14.2 QEMU gate
exercises these exact operations and requires `[S14.2A]` and `[S14.2B]`
markers. This remains a deterministic foundation boundary: ICMPv6 Neighbor
Discovery packets, Duplicate Address Detection, router discovery, DHCPv6,
live IPv6 sockets, and physical IPv6 qualification remain open. See the
[Stage 14.2B audit](audit-stage14-2b-ipv6-neighbor-foundation-2026-10-06.md).

The bounded parser now accepts only ICMPv6 Router/Neighbor Solicitation and
Advertisement envelopes with code zero, valid fixed bodies, non-multicast
targets, and complete non-zero-length options. It caps input at 1 KiB and does
not retain packet ownership. The 1/2/4-CPU gate exercises valid and malformed
messages. IPv6 pseudo-header checksum verification, Duplicate Address
Detection, router state, DHCPv6, live IPv6 sockets, and physical IPv6
qualification remain open.

See the [Stage 14.2C audit](audit-stage14-2c-icmpv6-neighbor-parser-2026-10-06.md).

The parser now has a checked entry point that authenticates the ICMPv6
pseudo-header using the IPv6 source/destination, payload length, next-header
value, and packet checksum. Tampered packets are rejected before Neighbor
Discovery parsing. Router state, Duplicate Address Detection, DHCPv6, live
IPv6 sockets, and physical IPv6 qualification remain open.

See the [Stage 14.2E audit](audit-stage14-2e-icmpv6-checksum-2026-10-06.md).

## Stage 14.2F — Neighbor cache and DAD state (2026-10-06)

The bounded state layer now accepts only checksum-validated Neighbor
Advertisements, stores up to eight expiring link-layer records, and exposes
explicit Duplicate Address Detection pending/unique/duplicate outcomes. It
does not yet transmit probes or integrate live interface state. DHCPv6, live
IPv6 sockets, and physical IPv6 qualification remain open.

See the [Stage 14.2F audit](audit-stage14-2f-neighbor-state-2026-10-06.md).

Next: connect Neighbor Discovery state to bounded live IPv6 interface traffic.

## Stage 14.2G — SLAAC lifecycle (2026-10-06)

The bounded SLAAC lifecycle now derives an address from an accepted /64
prefix, tracks tentative/preferred/deprecated/expired/duplicate states, and
applies DAD completion plus valid/preferred lifetimes. Live router egress,
DHCPv6, sockets, and physical qualification remain open.

## Stage 14.2H — DHCPv6 protocol foundation (2026-10-06)

The bounded DHCPv6 foundation validates supported message types, 24-bit
transaction identifiers, complete non-zero-length options, and bounded
serialization. It is protocol framing only: live lease exchange, retransmit
timers, address installation, and persistent lease state remain open.

## Stage 14.2I — DHCPv6 client state machine (2026-10-06)

The bounded DHCPv6 client state machine validates Solicit/Advertise/Request/
Reply progression, binds replies to the transaction and server DUID, and
applies bounded retry timing. It does not perform live lease installation or
persistent configuration.

## Stage 14.2J — IPv6 interface ingress boundary (2026-10-06)

The ingress coordinator verifies ICMPv6 checksums before dispatching Router
Advertisement, Neighbor Advertisement, and Neighbor Solicitation packets into
bounded interface state. It emits explicit events for accepted traffic, DAD
conflicts, and rejected packets. No packet slice is retained and no physical
NIC activation is claimed.

See the [Stage 14.2J audit](audit-stage14-2j-ipv6-interface-ingress-2026-10-06.md).

Next: bounded IPv6 interface address configuration and controlled egress.

## Stage 14.3 — WovenNet socket API boundary (2026-10-06)

Socket endpoint admission now rejects zero-port, unspecified, and IPv4
multicast destinations before smoltcp state mutation. Packed IPv4 endpoint
decoding and encoding have a deterministic round-trip contract, while the
existing owner/generation socket lifecycle remains unchanged. This is an API
boundary; live IPv6 sockets, routing policy, and physical NIC qualification
remain open.

See the [Stage 14.3 audit](audit-stage14-3-socket-api-2026-10-06.md).

Next: bounded routing-table ownership above the accepted socket API.

## Stage 14.4 — WovenNet routing-table foundation (2026-10-06)

The bounded routing table now supports eight generation-tagged routes, rejects
invalid prefixes and zero gateways, performs longest-prefix matching, and uses
metric then generation tie-breaking. Removal requires the exact route handle,
so stale removal cannot delete a replacement. Live route installation into the
network interface and physical NIC qualification remain open.

See the [Stage 14.4 audit](audit-stage14-4-routing-table-2026-10-06.md).

## Stage 14.5A — WovenGuard firewall policy foundation (2026-10-07)

The bounded WovenGuard firewall policy foundation is accepted. It supports
inbound, outbound, and forward directions; allow/deny actions; protocol,
dual-stack CIDR, and port matching; deterministic priority/ID ordering; a
default policy; bounded rule storage; and synchronized global policy access.
This foundation is policy-only: live packet-path enforcement remains Stage
14.5B work.

The Stage 14.5A acceptance marker is
`[S14.5A] firewall policy foundation PASSED`, with the inherited WovenNet
networking gates retained. GitHub Actions run #1267 passed on commit
`f8b79ac7`.

Next: Stage 14.5B live packet-path enforcement without weakening the accepted
DHCP, IPv4/IPv6, socket, or routing behavior.

## Stage 14.5B — WovenGuard live packet enforcement (2026-10-07)

Stage 14.5B is accepted. WovenGuard now evaluates real Ethernet-frame traffic
at the virtio network boundary: inbound denial occurs before an RX token is
returned to smoltcp, and outbound denial occurs after frame construction but
before hardware submission. The shared parser derives IPv4/IPv6 TCP, UDP,
ICMP, and ICMPv6 policy metadata while preserving non-IP Layer-2 control
traffic.

Live enforcement has an explicit activation lifecycle. It remains disabled
during network bootstrap, is enabled for controlled policy enforcement, and
does not replace the policy's default-deny semantics with a permissive default.
This prevents DHCP/bootstrap deadlock while keeping firewall activation an
intentional security transition.

The Stage 14.5B acceptance marker is
`[S14.5B] live packet enforcement PASSED`. GitHub Actions run #1277 passed
the dedicated build, Clippy `-D warnings`, inherited Stage 14.1-14.5A network
regressions, and QEMU 1/2/4-core acceptance on commit `1d1ce646`.

Next: extend WovenGuard beyond the accepted virtio live boundary with explicit
policy-management authority and transport coverage before claiming a
production firewall.

## Stage 14.5C — WovenGuard firewall policy authority (2026-10-07)

Stage 14.5C is accepted. Firewall administration is separated from ordinary
network use through the dedicated `NetworkAdmin` capability. Normal users keep
`NetworkIo` for socket traffic but cannot add, remove, replace, or otherwise
change global firewall policy, the default action, or live-enforcement state.
`NetworkAdmin` is available only within the Kernel/SystemService domain
ceiling.

The acceptance marker is `[S14.5C] firewall policy authority PASSED`.
GitHub Actions run #1287 passed the dedicated build, Clippy `-D warnings`,
inherited Stage 14.1-14.5B regressions, and QEMU 1/2/4-core acceptance on
commit `b81b3a76`.

Next: complete firewall transport/path coverage and parser hardening before
adding stateful connection tracking.

## Stage 1–5 long-name mutation hardening — 2026-10-01

The FAT32 create/overwrite and rename paths now compare the user-visible long
name before selecting an 8.3 alias. Existing long-name files are overwritten
through their existing directory entry, and attempts to create a directory or
rename another file onto that name fail closed. The strengthened FAT32
self-test and complete Stage 1–5 QEMU gate passed on 1/2/4 CPUs with exit 33.
See [the hardening audit](audit-stage1-5-long-name-collision-2026-10-01.md).

This is a bounded software hardening increment, not a claim that the remaining
hardware qualification, DMA storage completion, or full data rollback gaps are
closed.

## Stage 1–5 durable data journal — 2026-10-01

FAT32 persistence now uses a bounded on-volume prepared/committed data journal
to recover interrupted single-file replacements, including restoration of the
previous bytes or removal of a newly-created target. Payload tampering fails
checksum validation. The journal self-test and 1/2/4-CPU QEMU gate passed.
See [the data-journal audit](audit-stage1-5-data-journal-2026-10-01.md).

Multi-file atomicity, larger-than-VFS files, and physical power-loss testing
remain open.

The journal now writes version 2 records with the intended replacement length
and checksum. Mount recovery validates committed records before retiring them
and restores the previous bytes if the committed target does not match.
Version 1 records remain readable for compatibility. Multi-file atomicity and
physical power-loss testing remain open.

## Stage 1–5 metadata capacity extension — 2026-10-01

The ownership sidecar preserves the WMD1 four-sector ABI and now uses a
versioned WMD3 extension in eight additional reserved sectors when the volume
geometry permits it. Capacity increases from 84 to 252 records without
relocating or rewriting legacy metadata. The strengthened self-test and
1/2/4-CPU QEMU gate passed with exit 33. See the
[metadata-capacity audit](audit-stage1-5-metadata-capacity-2026-10-01.md).

The table remains intentionally bounded; a dynamically growing metadata
format and full multi-operation transaction log are still future work.

Production completion is tracked separately from bounded foundation acceptance
in [the Stage 1–12 gap audit](stage1-12-production-gap-audit.md). No stage is
fully complete while a roadmap requirement remains open.

## Physical qualification tooling — 2026-10-01

The read-only physical-probe image now records CPU features, ACPI/ECAM state,
PCI class information, and every retained PCI function before stopping ahead
of driver activation. Its 1/2/4-CPU QEMU inventory smoke passed. This improves
lab evidence collection but does not close physical storage, DMA, interrupt,
radio, or unclean-shutdown qualification. See the
[physical inventory audit](audit-physical-inventory-2026-10-01.md).

Authoritative development sequence: [supplied Stage 10.7–36 roadmap](master-development-roadmap.md).

## TCP close/drain preservation repair - 2026-09-24

This-machine preflight exposed an intermittent 4-CPU Stage 10.7 failure:
last-reference release removed a TCP socket with 16 bytes still queued.
Graceful bounded retirement now preserves that transport. Ten corrected
4-CPU repeats passed, including queued-close cases; the full 75-boot Stage
10.7 preservation gate passed. Related 10.8/10.9/13.9 checks on 1/2/4 CPUs
and normal release shell/SMP smoke also passed. Assertions and existing test
timeouts are unchanged; cleanup now requires transport slots at baseline. See
[the TCP repair audit](audit-tcp-close-drain-2026-09-24.md).

## Physical integration request - 2026-09-24

Physical integration is incomplete. The latest user instruction selects this
HP Pavilion x360 as the actual test machine, superseding the earlier AX200
target choice. Its adapter is AX201 `8086:A0F0`, subsystem `8086:0074`, revision
`20`. Native AX201 startup and transport remain unimplemented; adding its PCI
ID to the AX200 activation list is not a valid implementation.

The authorized Kingston USB was cleared and written with the verified inventory
image. An elevated helper read back all 8,454,144 image bytes and matched the
SHA-256. Its new GPT layout contains an 8 MiB EFI system partition. The internal
system disk was not written. Secure Boot is enabled and the image has no PE
Authenticode certificate; native boot requires a local firmware decision.
Windows C: is fully encrypted with BitLocker protection on, so the recovery key
must be available before changing firmware security settings. Neither BitLocker
nor firmware settings were changed, and no reboot or physical test has run.
The inventory image passed 1/2/4-CPU configured QEMU boots plus a no-UART
framebuffer check. No AP startup or physical radio is tested by that mode.
See [the AX201 machine audit](audit-ax201-machine-2026-09-24.md).

The runtime firmware loader now validates the complete AX200 section layout
before DMA allocation and copies payloads into an unpublished owned context.
It skips separators and init-image records and preserves existing DMA bounds.
Build, warning-denying host/kernel/feature Clippy, all 35 Rust tests and all
54 Wi-Fi markers on 1/2/4 CPUs passed for this loader increment.
See [the runtime-loader audit](audit-ax200-runtime-loader-2026-09-24.md).

A real AX200 firmware container exposed parser defects. The corrected
allocation-free TLV parser passes seven new host tests and the real-image
probe; all 54 Wi-Fi markers still pass on 1/2/4 CPUs. Build, host-test and
kernel Clippy, and all 31 Rust tests pass. The full Stage 10.7 preservation
gate passed after the heap lint cleanup: nine Python tests and 75 QEMU
boots, plus three focused Wi-Fi boots (78 total for this prerequisite).
See [the physical preflight audit](audit-physical-wifi-preflight-2026-09-24.md).
This is prerequisite work, not physical firmware/IRQ/DMA/RF acceptance.

## Current continuation - 2026-09-24

Base commit `0563d0b` introduced the Stage 13.10AC deferred AX200 interrupt
service candidate. This continuation passed preservation and bounded QEMU
acceptance; it does not establish production completion of Stage 13.

- Ordinary build and host/freestanding-kernel Clippy passed with warnings
  denied; 24 Rust host tests and nine Python harness tests passed.
- The complete Stage 10.7 preservation gate passed, exit code 0: 75 QEMU boots
  covering 1/2/4 CPUs, live networking, and async block/file/UDP/TCP.
- Stages 1-5, 10.8-10.9, 11.1-11.5, 12.1-12.5, and 13.1-13.9 passed
  per-feature freestanding Clippy and 1/2/4-CPU QEMU (66 additional boots).
  Wi-Fi required all 54 markers through 13.10AC on every CPU configuration.
- CPU hotplug passed on 2/4 CPUs. The normal release build and 4-CPU
  shell/PS2/IOAPIC/SMP smoke passed. Total: 144 successful QEMU boots.

The continuation corrected feature boundaries, a misplaced audio check in
USB HID's error branch, and a nested Cargo release-build artifact-lock
conflict in the shell harness. Failed/interrupted attempts remain preserved.
See [the continuation audit](audit-stage13-10ac-continuation-2026-09-23.md)
for exact evidence directories and validation scope.

Next prerequisites remain production Wi-Fi worker/lifecycle integration,
secure entropy provisioning, physical AX201 firmware/IRQ/DMA/RF qualification,
and the earlier unfulfilled roadmap requirements. Do not advance to a later
stage or treat these synthetic Wi-Fi results as working physical Wi-Fi.

## Stage 6 — accepted on 2026-09-16

The bounded SMP foundation passed the complete release matrix: warning-denying
kernel/host Clippy, all Rust host tests, 1/2/4-CPU memory/storage/network
gates, legacy PIC fallback, 4-CPU release gates, and shell/SMP smoke. See
[the Stage 6 audit](audit-stage6-2026-09-16.md).

General multicore userspace, unrestricted concurrent device/filesystem service
throughput, multi-node NUMA page-placement qualification, non-contiguous
hotplug hardware qualification, APIC-ID-above-255 hardware qualification, and
broader cross-layer lock-path qualification and priority inheritance
remain explicit deferred requirements in the Stage 6 contract. CPU-domain
placement, audited Ready-state I/O service migration, and the x2APIC MSR transport are
implemented; bounded contiguous-prefix AP offline/re-online control is also
implemented; interrupt-safe scheduler, IPC, WovenGuard, teardown, and worker
locks now enforce bounded rank and nesting
checks, and VirtIO network DMA uses an allocator-reserved physically
contiguous arena; see the [NUMA/x2APIC audit](audit-stage6-numa-2026-09-16.md),
[hotplug audit](audit-stage6-hotplug-2026-09-16.md), and
[lock-order audit](audit-stage6-lock-order-2026-09-16.md) and
[DMA audit](audit-stage6-dma-2026-09-16.md).

The CPU lifecycle follow-up supports a non-contiguous online mask in QEMU:
CPU 1 can park while CPU 3 continues scheduling work and acknowledging TLB
shootdowns, then rejoin its stable slot. Offline preparation validates every
task move before committing any; draining CPUs reject new placement. See the
[non-contiguous hotplug audit](audit-stage6-noncontiguous-hotplug-2026-09-17.md).
Physical non-contiguous hotplug, high APIC-ID, and multi-node NUMA hardware
qualification remain open.
The final source passed the complete release matrix and dedicated twice-cycled
2/4-CPU hotplug gates after this follow-up.
The cancellation follow-up distinguishes unclaimed requests from AP-owned
transitions, rearms rejected states, and keeps further hotplug disabled if an
AP never finishes a claimed transition. Forced AP rejections and unclaimed
timeouts recover and retry in the 2/4-CPU QEMU gates; broad concurrent-work
and physical-fault stress remains open.

The 2026-09-17 lock follow-up made the file-frame cache IRQ-safe, moved
fork/dup reference retention outside the process-table lock, and added
generation-tagged VFS and pipe handles to reject stale slot reuse. The VFS
read and materialization paths now release registry/open-description guards
for disk I/O and revalidate node identity, version, backing, and shared seek
position before committing results. Both VFS tables now use ranked IRQ-safe
locks. See the
[lock-order audit](audit-stage6-lock-order-2026-09-16.md). The full release
matrix and twice-cycled 2/4-CPU hotplug gates passed after these changes.
The pipe follow-up also uses scheduler-latched wakeups and rejects full waiter
tables, closing a lost-wakeup window under concurrent readers and writers.
The global heap metadata lock is now rank-50 IRQ-safe; page mapping happens
before that guard is taken during boot. Its earlier full release and 2/4-CPU
hotplug gates passed. The follow-up makes boot capacity RAM-scaled from
256 KiB to 8 MiB, raises live-allocation metadata to 2,048 entries, recovers
alignment padding, and coalesces freed intervals. A partial kernel map now
rolls back. Runtime growth beyond the eager mapping remains open.
See the [Stage 6 heap audit](audit-stage6-heap-2026-09-17.md).
The final heap change passed the full release matrix and twice-cycled 2/4-CPU
hotplug gates.
The heap metadata follow-up replaces the fixed live-object and free-interval
tables with per-span headers and an in-place coalescing free list. Host tests
and a 4,096-object QEMU probe cover the former 2,048-object ceiling; mapped
runtime growth and broad allocator throughput remain open. See the
[heap metadata audit](audit-stage6-heap-metadata-2026-09-17.md).
The final metadata source passed the full 33-check release matrix and the
twice-cycled 2/4-CPU hotplug gates.
Runtime heap growth now maps and publishes 256 KiB chunks outside the heap
guard. Rank-free, IRQ-enabled allocations can grow on failure; BSP idle maps
ahead for guarded callers when free space is low. QEMU exercises growth after
SMP startup and reserve maintenance. Sudden large guarded allocations,
low-memory behavior, and multicore throughput still need qualification; see
the [heap growth audit](audit-stage6-heap-growth-2026-09-17.md).
The final growth source passed the complete 33-check release matrix and
twice-cycled 2/4-CPU hotplug gates.
The frame allocator now reserves a bitmap in each usable physical range and
uses it to track allocations, reject double frees, and recover returned
frames beyond its 4,096-entry hot cache. See the
[frame reclamation audit](audit-stage6-frame-reclamation-2026-09-17.md).
The QEMU memory gate exercises 4,352 returned frames; physical high-RAM and
NUMA latency qualification remains open.
The final frame-reclamation source passed the full 33-check release matrix
and dedicated twice-cycled 2/4-CPU hotplug gates.
The bounded device, keyboard decoder, journal, swap-state, mount-record, and
key-vault tables now use IRQ-safe locks. Keyboard input preserves the caller's
pre-lock interrupt state for early-boot polling; Stage 1-5 journal, Stage 12.3
key-vault, Stage 12.5 mount-record, and normal PS/2 shell gates passed.
The full release matrix and dedicated twice-cycled 2/4-CPU hotplug gates also
passed after the keyboard interrupt-state correction.
The FAT32 clean-page cache now uses a rank-20 IRQ mutex only for short metadata
and page-copy sections; physical page loads run after releasing it, with an
invalidation epoch preventing stale publication. Host, full release, and
2/4-CPU hotplug gates passed. Cross-layer FAT32 mutation transactions remain
open for unrestricted concurrent filesystem service.
The shell current-directory state now has a short rank-10 IRQ guard, with a
fixed-buffer copy released before console output or allocation. The terminal
has a rank-10 preemption guard that keeps device IRQs live during rendering,
and syscall writes restore live IRQs around full-frame operations. Lock-order
tracker updates remain interrupt-atomic. This closes the audited terminal and
shell compatibility-lock gap, but not the other Stage 6 production gaps above.
The final source passed the complete release matrix and twice-cycled 2/4-CPU
hotplug gates; details are in the lock-order audit.
The network runtime and VirtIO transport now declare ranks 20 and 30 for
their existing runtime-to-transport nesting. No kernel `IrqMutex::new` call
sites remain; this is rank coverage, not proof of every cross-subsystem path.
The ranked network change passed the full release matrix and twice-cycled
2/4-CPU hotplug gates.

## Stage 10.7 — accepted on 2026-09-16

The imported recovery source is preserved in Git commit `49e789d`. That commit
records the baseline only and does not certify acceptance.

The audit corrected worker starvation, local-preemption lock safety, readiness
observation, host-exchange validation and evidence retention. The complete gate
ended with `=== STAGE 10.7 ACCEPTANCE: PASS ===` and host exit status 0.

| Gate | Result |
| --- | --- |
| `cargo build` | Passed |
| Host and freestanding-kernel Clippy with `-D warnings` | Passed |
| Rust host regressions | 13 passed |
| Python host-harness regressions | 4 passed |
| Memory/scheduler/pager/IPC/security preservation | 1 CPU: 10/10; 2 CPUs: 30/30; 4 CPUs: 20/20 |
| Live DHCP/DNS/ICMP/UDP/TCP | 1/2/4 CPUs passed |
| Stage 10.4 asynchronous block I/O | 1/2/4 CPUs passed |
| Stage 10.5 asynchronous file I/O | 1/2/4 CPUs passed |
| Stage 10.6 asynchronous UDP | 1/2/4 CPUs passed |
| Strengthened Stage 10.7 asynchronous TCP | 1/2/4 CPUs passed |

Total: 75 QEMU boots passed in the final full chain. Each boot required debug-exit
status 33 and its required markers; network harnesses also verified host traffic.
Final build/lint/host checks also passed and are recorded under
`audit-artifacts/final-static/`. The accepted-stage commit follows the imported
baseline as a separate Git commit; use `git log --oneline` to identify it.

## Stage 10.8 — accepted on 2026-09-16

Completion ports, operation association, batch poll/wait, bounded reservations,
cancellation events, generation retirement and timeout-aware scheduler blocking
are implemented. The full gate passed with exit code 0: all 75 Stage 10.7 boots
plus completion-port boots on 1/2/4 CPUs (78 total). The latter include Ring-3
copyout retry, measured timeout, teardown, SMP producers and consumer wakeup.
Build, warning-denying host/kernel/probe Clippy, 18 Rust tests and four Python
harness tests passed. Final host tests include waiter notification and port quota.

Full transcript: `audit-artifacts/acceptance-20260916-085314-575/acceptance-output.txt`.
Focused serial evidence is retained in timestamped `audit-artifacts/stage10.8-*`
directories. See [ABI contract](completion-port-abi.md) and
[audit](audit-stage10-8-2026-09-16.md).

## Stage 10.9 — accepted on 2026-09-16

Timers, deadlines, asynchronous event objects, cancellation and sleep-until
are implemented under bounded process ownership. The 1/2/4-CPU QEMU gate,
build, warning-denying Clippy and Rust regressions passed. See
[audit](audit-stage10-9-2026-09-16.md).

## Stage 10.10 — accepted on 2026-09-16

The aggregate gate validates the Stage 10.8 completion architecture together
with Stage 10.9 timers/events on 1, 2, and 4 CPUs. All six boots passed with
exit 33 and retained serial evidence. Stage 10 is closed; Stage 11 follows.

## Stage 11.1 — accepted on 2026-09-16

PID lifecycle, parent/child ownership, exit status, wait semantics, process
groups, isolated resources and teardown passed build, Clippy, host tests, and
1/2/4-CPU QEMU validation. See [audit](audit-stage11-1-2026-09-16.md).

Next: Stage 11.2 per-process threads and join/TLS semantics.

## Stage 11.2 — accepted on 2026-09-16

Generation-safe thread IDs, owner-checked join, termination status, TLS and
thread-local errno state passed build, Clippy, host tests, and 1/2/4-CPU QEMU
validation. See [audit](audit-stage11-2-2026-09-16.md).

Next: Stage 11.3 structured notifications and lifecycle events.

### Stage 11.2 scheduler-backed thread follow-up — accepted increment on 2026-10-14

The bounded scheduler-backed user-thread path now has an end-to-end Ring-3
create/exit/join acceptance. It uses private NX stacks, copied initial-exec
TLS, stable process ownership, generation-safe IDs, and post-switch resource
reclamation. Build, warning-denying Clippy, host tests, and 1/2/4-CPU QEMU
passed; see [audit](audit-stage11-2-scheduler-threads-2026-10-14.md).

This is an implementation increment, not closure of every Stage 11 thread
requirement. Sibling cancellation/teardown, shared libraries, symbol
resolution, dynamic TLS, and hardware qualification remain open.

## Stages 11.3–11.5 — accepted on 2026-09-16

Structured notifications, the `libwoven` userspace API boundary, and hardened
ELF/W^X loader validation passed their individual 1/2/4-CPU QEMU gates,
freestanding Clippy, and host tests. See [audit](audit-stage11-3-5-2026-09-16.md).

The follow-up ASLR pass now randomizes production ELF, stack and mmap bases
with a deterministic `qemu-test` switch; see [audit](audit-aslr-2026-09-16.md).

The 2026-10-05 loader-boundary follow-up rejected `PT_INTERP`, `PT_DYNAMIC`,
`PT_TLS`, and `PT_GNU_RELRO` program headers instead of silently accepting
binaries whose dynamic-linking, TLS, or RELRO semantics were not implemented
at that point. This was a fail-closed production-safety boundary; later
follow-ups narrow the remaining unsupported surface.
See [audit](audit-stage11-loader-boundary-2026-10-05.md).

The 2026-10-14 RELRO follow-up closes the static RELRO permission transition:
a single validated `PT_GNU_RELRO` range is protected read-only after loading,
ASLR relocation carries the range, and COW write-fault bookkeeping refuses to
reattach write authority to it. Valid/invalid parser cases and the complete
Stage 11.5 1/2/4-CPU gate passed. Shared libraries, symbol-based relocation,
and physical/hardware qualification remain open. See
the [RELRO audit](audit-stage11-relro-2026-10-14.md).

The 2026-10-14 static TLS follow-up adds a bounded initial-exec `PT_TLS`
contract: the loader validates and maps one initial TLS image, task context
switches install its FS base, and fork/exec preserve the ownership and COW
rules. Shared-library TLS, TLS module IDs, multi-thread TLS semantics, and
physical/hardware qualification remain open. See the
[static TLS audit](audit-stage11-static-tls-2026-10-14.md).

The 2026-10-14 dynamic-relocation follow-up accepts bounded self-contained
ET_DYN images with R_X86_64_RELATIVE plus locally defined symbol RELA entries.
SysV hash/symbol/string metadata is bounded and validated; relocations are
applied before W^X and RELRO finalization. The inherited 1/2/4-CPU Stage
11.3–11.5 matrix passed with an end-to-end mapped-image local-symbol
relocation self-test. Shared-library dependency mapping, cross-image symbol
resolution, versioning, lazy binding, and dynamic TLS remain fail-closed. See
the [dynamic relocation audit](audit-stage11-dynamic-relocations-2026-10-14.md).

## Stage 12.1 — accepted on 2026-09-16

The VFS 2.0 typed interface and SystemVfs adapter passed build, freestanding
Clippy, and QEMU validation on 1/2/4 CPUs. See
[audit](audit-stage12-1-2026-09-16.md).

Next: Stage 12.2 WovenFS metadata and crash-consistency design.

## Stage 13.1 — accepted on 2026-09-16

The WovenDriver manager passed device matching, binding, suspend/resume,
build, freestanding Clippy, and 1/2/4-CPU QEMU validation. See
[audit](audit-stage13-1-2026-09-16.md).

## Stage 13.2 — topology/ownership increment implemented; full acceptance open (2026-09-27)

The PCIe foundation now includes bounded generation-tagged function/bridge
topology, explicit single-owner claims, generation-tagged logical MMIO BAR
leases, teardown-before-reuse invalidation, and bridge bus-window parentage.
The dedicated preservation gate requires focused host tests, warning-denying
kernel Clippy, runtime-harness regressions, and 1/2/4-CPU QEMU evidence.

Full Stage 13.2 acceptance remains open for physical BAR qualification,
bridge interoperability beyond the QEMU fixture, MSI qualification beyond the
QEMU SATA path, MSI-X delivery, PCIe link/power hotplug, and device-specific
reset/recovery. The software BAR probe, bounded
allocation/rebalance, bridge routing transaction, interrupt ownership paths,
and hotplug reconciliation are covered by the follow-ups below. Raw legacy
MMIO mappings remain shared and non-revocable; owner-scoped mappings are
covered by the follow-up below.

The WovenDriver integration item now has a bounded acceptance probe. The
Stage 13.2 gate claims one discovered function for a synthetic feature-gated
owner, retains its programmed MMIO BAR leases, and releases subordinate
resources before function ownership. The 1/2/4-CPU preservation gate passed;
see [the lifecycle audit](audit-stage13-2-driver-lifecycle-2026-10-13.md).
This does not close the remaining physical rebalance, hardware interrupt
qualification, hotplug, or MMIO mapping-revoke requirements.

The follow-up adds owner-scoped page mappings: raw legacy MMIO mappings remain
shared and non-revocable, while PCI driver mappings carry an owner token and
are revoked after interrupt quiescence and before BAR/function release. The
1/2/4-CPU gate passes and proves same-page remap after teardown. QEMU's only
candidate firmware BAR is outside the direct-map range and is rejected
fail-closed, so this is not physical device-BAR qualification. Remaining work
is MSI-X/physical interrupt qualification, PCIe link/power hotplug, and
valid-BAR hardware qualification beyond the QEMU fixture.

The acceptance probe now also runs the driver-aware no-removal rescan after
teardown and recomputes bridge routing before it finishes. The 1/2/4-CPU
matrix passed; this validates coordinator ordering but does not substitute for
physical PCIe insertion/removal qualification.

Initial PCI discovery now has the same admission boundary: `hal::init` uses
discover-and-route, bridge-bearing generations are not driver-admissible until
the rollback-capable bridge transaction succeeds, and routing/discovery failure
rejects later driver claims fail-closed. The 1/2/4-CPU preservation gate passed;
see the [admission-routing audit](audit-stage13-2-admission-routing-2026-10-13.md).
This validates ordering and failure policy, not physical bridge-window
interoperability on hardware absent from QEMU.

The ECAM configuration path now applies the mapped page offset for both reads
and writes. Before this repair, live ECAM discovery repeated the vendor/device
dword at every offset, masking real class, BAR, and capability behavior. The
corrected path is covered by the 1/2/4-CPU gate and the
[ECAM offset audit](audit-stage13-2-ecam-offset-2026-10-13.md).

The reconciliation boundary also clears admission before publishing a
refreshed inventory, eliminating the interval where a concurrent bind could
observe new topology before bridge programming completed. The updated 1/2/4
CPU matrix passed with warning-denying Clippy.

The firmware BAR rebalance path now retains successful reservations in bounded
persistent PCI resource state, rejects duplicate function admission, clears
and verifies BAR registers before releasing reservations, and refreshes the
published device state. The focused freestanding check and warning-denying
Clippy pass; see the [persistent BAR audit](audit-stage13-2-bar-resources-2026-10-13.md).

Persistent BAR teardown now verifies PCI command-space decoding is disabled,
clears the assigned BARs, refreshes inventory, and reroutes bridge windows
before releasing address reservations; failed verification retains the
allocation for retry. This prevents allocator reuse while a device could
still decode its old BAR assignment.

Normal PCI driver admission now invokes the BAR allocator for the safe
all-unassigned case, refreshes the published BAR view before leasing, rejects
mixed layouts fail-closed, and checkpoints persistent ownership through
unbind. Successful rebalance also reroutes the bridge hierarchy before any
MMIO lease is published. See the [automatic BAR admission audit](audit-stage13-2-bar-admission-2026-10-13.md).
QEMU still lacks a valid physical unassigned-BAR fixture, so the positive
hardware allocation path remains unqualified.

PCI rescans now report newly inserted functions through a bounded
`hotplug_rescan_report` while preserving the existing summary-only API. Full
driver-relevant identity changes at a BDF are detected; if replacement occurs
before old-owner quiescence, the entire batch fails closed before any teardown
configuration write. Persistent BAR reservations are released before unbound
generations are invalidated, and new functions are verified after topology
publication and bridge routing before being returned to driver matching;
unknown hardware is never auto-bound. See the [hotplug insertion audit](audit-stage13-2-hotplug-insertion-2026-10-13.md).
Physical PCIe link/power hotplug remains hardware-dependent.

The driver manager now provides bounded PCI identity matching and an explicit
match-then-bind API for insertion reports. Empty matchers are rejected and
unknown hardware remains unclaimed. See the [driver-matching audit](audit-stage13-driver-matching-2026-10-13.md).
Device-specific drivers, initialization policies, and physical PCIe
interoperability remain later qualification work.

The driver layer now exposes generic interrupt selection: MSI-X is attempted
when capability and retained BAR authority permit it, clean failures fall back
to MSI, and activation-retained leases never fall back or lose ownership. See
the [interrupt-selection audit](audit-stage13-2-interrupt-selection-2026-10-13.md).
Physical vector delivery remains unqualified on QEMU.

PCI driver suspend/resume now owner-checks the function, disables and verifies
command decoding, and, when the PCI Power Management capability is present,
checkpoints PMCSR, enters D3hot, restores D0, and restores the exact command
checkpoint before resuming. Missing checkpoints and failed transitions fail
closed. See the [PCI suspend/resume audit](audit-stage13-2-suspend-resume-2026-10-13.md).
Device-specific reset, D3cold, wake signaling, and physical power-management
qualification remain open.

The generic MSI programming path now snapshots and restores the original
capability/message state on every configuration failure, including a failed
initial disabled-header write. Every header, address, and data write is
read-back verified; `RestoreFailed` is returned when repair itself cannot be
verified. Vector and interrupt-mode ownership remain retained until successful
quiescence. Corrected ECAM offset handling now decodes live PCI classes and
capabilities correctly; the QEMU SATA fixture exercised generic MSI activation
and ordered teardown on the 1/2/4-CPU preservation matrix. See the
[MSI rollback audit](audit-stage13-2-msi-rollback-2026-10-13.md). Physical
MSI qualification beyond this QEMU fixture and MSI-X table/vector delivery
remain hardware-dependent.

MSI-X activation now asserts Function Mask and keeps MSI-X disabled before any
table MMIO. Table reads and programming use owner-scoped MMIO mappings and
release those mappings after each bounded volatile operation. If programming
has made a table entry device-visible but cleanup fails, the `MsixLease` is
retained for ordered teardown rather than losing the vector obligation. The
Stage 13.2 probe now also scans disposable QEMU PCI fixtures, activates a real
MSI-X lease when its table BAR is admissible, and verifies the bounded
high-MMIO fallback mapping; the 1/2/4-CPU preservation matrix passed with
those fixtures present. See the
[MSI-X mapping audit](audit-stage13-2-msix-mapping-2026-10-13.md). Physical
MSI-X delivery, PCIe hotplug, and physical device lifecycle remain
unqualified because QEMU does not provide those physical conditions.

High 64-bit BAR pages no longer depend on the low fixed virtual alias. The
bounded high-MMIO window retains each mapping's virtual address and uses that
address for revocation/release; see the
[high-MMIO audit](audit-stage13-2-high-mmio-2026-10-13.md).

Stage 13.3 NVMe now passes its ordered 1/2/4-CPU QEMU acceptance gate. The
driver identifies a namespace, creates admin and I/O queues, performs
write/read/flush against a disposable image, returns owned DMA pages on failed
initialization, and restores the original PCI command after bounded controller
quiescence. See the [Stage 13.3 NVMe audit](audit-stage13-3-nvme-2026-10-13.md).
This closes the emulated synchronous foundation only; interrupt-backed
completion, reset/recovery, hotplug removal, multi-namespace policy, and
physical NVMe qualification remain open.

Stage 13.8 HDA/WovenAudio now passes its ordered 1/2/4-CPU QEMU gate after
correcting register page-offset handling in all HDA MMIO helpers. The gate
covers codec discovery, topology, playback/capture streams, mixer state, and
the WovenAudio API boundary. See the [HDA MMIO audit](audit-stage13-8-hda-mmio-2026-10-13.md).
Physical codec interoperability and interrupt/DMA fault recovery remain open.

The local Stage 14.1–14.5D feature gates now pass build/lint and live network
acceptance on 1/2/4 CPUs. A shared PCI `expect(dead_code)` guard was extended
through the later feature family after Stage 14.1 exposed that it could not
compile cleanly under warning denial. See the [Stage 14 feature-gate audit](audit-stage14-feature-gates-2026-10-13.md).

## Stages 12.2–12.5 — accepted on 2026-09-16

WovenFS metadata/integrity, volume-integrity envelope boundaries, snapshots,
and storage-management inventory each passed their 1/2/4-CPU gates. See
[audit](audit-stage12-2-5-2026-09-16.md). Production AEAD now passes the
isolated 1/2/4-CPU gate with a bounded revocable key vault; see
[audit](audit-volume-crypto-2026-09-16.md). The follow-up integration now
covers measured provisioning, authenticated persistent wrapped keys,
successor-generation rotation, and encrypted mount-record I/O. A TPM/firmware
measured-root backend, physical power-loss evidence, and deployment key-slot
policy remain open.

## Stage 7.1.5 - accepted on 2026-09-16

The scheduler/pager watchdog and termination lifecycle gate passed in full:
50 one-CPU memory runs, 100 two-CPU runs, 50 four-CPU runs, and live
DHCP/DNS/ICMP plus host-verified UDP/TCP at 1/2/4 CPUs. See
audit-stage7-1-5-timeout-2026-09-16.md for the bounded lifecycle handoff
correction and retained serial evidence.

## Roadmap Stage 13.10A — Bluetooth HCI core accepted on 2026-09-30

The hardware-independent Bluetooth HCI foundation is accepted. Bounded command encoding, Command Complete parsing, controller command-credit state, opcode/status validation, and deterministic self-tests passed the dedicated 1/2/4-CPU QEMU gate with exit 33 in GitHub Actions run 36718370038. See [acceptance audit](../AUDIT-STAGE13.10A-BLUETOOTH-HCI-ACCEPTANCE.md).

Historical Wi-Fi labels 13.10A–AC are Stage 13.9 AX200/WovenWiFi extensions; roadmap Stage 13.10 is Bluetooth. Stage 13.10A does not claim USB transport or physical Bluetooth hardware support.

Next: Stage 13.10B Bluetooth USB HCI transport over xHCI.


## Roadmap Stage 13.10B — Bluetooth USB HCI transport contract accepted on 2026-09-30

The xHCI-owned Bluetooth USB transport contract is accepted at the descriptor/protocol boundary. Stage 13.10B recognizes the USB Wireless Controller / RF Controller / Bluetooth Primary Controller interface (E0/01/01), validates the interrupt-IN HCI event endpoint plus bulk-IN/bulk-OUT ACL endpoints, and constructs the class control-request setup packet used to carry HCI commands. The hardware-independent `bluetooth_hci.rs` protocol layer remains separate from USB transport semantics.

GitHub Actions run 36722602921 passed at source SHA `0ba3ce257f93775afe67ed975f8549fcdbb78506`, including lint/host regressions, the 1/2/4-core boot matrix, Stage 13.3–13.9 regression gates, and the Stage 13.10 runtime gate. The runtime harness requires `[S13.10B] Bluetooth USB HCI transport contract: PASSED`; the workflow step label still says Stage 13.10A and should be renamed in the next acceptance-maintenance change.

This acceptance is a transport-contract milestone, not a physical-controller claim. Live xHCI endpoint/ring ownership, HCI command/event transfer execution, ACL transfer execution, controller initialization, discovery, L2CAP, pairing/security, ATT/GATT, lifecycle/hotplug, and physical hardware qualification remain open.

Next: Stage 13.10C live USB HCI transfer execution and controller initialization boundary.


## Roadmap Stage 13.10C — bounded HCI transfer execution contract accepted on 2026-09-30

The Bluetooth command/event transaction boundary is accepted. The HCI core now serializes one outstanding command against controller credits, rejects a second command while one is in flight, requires the matching Command Complete opcode, and restores readiness only after successful completion. The xHCI Bluetooth layer also exposes bounded command, interrupt-event, ACL-IN, and ACL-OUT transfer plans derived from the accepted Stage 13.10B interface/endpoint contract.

GitHub Actions run 36729724615 passed at source SHA `5aa5628ad815ef2f04e0e1b3a150dad0bc30b355`. The dedicated “Stage 13.10C Bluetooth HCI transfer 1/2/4-core acceptance” gate and the complete Stage 13.3–13.9 regression chain passed. Preserved release-validation artifact digest: `sha256:b9bb2fa1d58274d268f069b44440844adbb87e51f3a04e0547d8868b5080dcf0`.

This milestone does not yet claim a physical Bluetooth controller or live USB transfer completion. Dedicated xHCI Bluetooth endpoint contexts/rings and DMA buffers, live control-OUT HCI command submission, interrupt-IN event reception, ACL execution, Reset/Read Local Version against hardware, teardown/recovery, and physical qualification remain open.

Next: bind the accepted transfer plans to dedicated xHCI rings/DMA ownership and execute the controller-initialization sequence.


## Roadmap Stage 13.10D — live USB HCI mechanics and controller initialization accepted on 2026-09-30

Stage 13.10D materializes the Bluetooth USB transport contract as xHCI-owned state: endpoint-zero control-OUT-with-data for HCI commands, dedicated interrupt-IN event and bulk-IN/bulk-OUT ACL endpoint contexts, separate transfer rings and DMA buffers, direction-aware DCI mapping, and bounded event/ACL execution primitives. The hardware-independent HCI layer adds a fail-closed controller initializer that requires Reset Command Complete before Read Local Version and reaches Ready only after the matching second completion.

GitHub Actions run 36747805657 passed at exact source SHA `41e009543e01afc6a348122ff3a2c4f0be16e351`. The dedicated “Stage 13.10D Bluetooth controller init 1/2/4-core acceptance” step passed together with the complete regression chain. Preserved release-validation artifact digest: `sha256:a245914f91143eb8f4d4b5aa6747d3d5bd07230c6dfa6d650455eb27ba52cae1`.

This acceptance proves the bounded transport/initialization architecture under deterministic CI; it does not claim physical Bluetooth-radio qualification because the QEMU acceptance device is not a Bluetooth controller. Physical USB-controller execution, teardown/recovery, hotplug, and radio qualification remain explicit hardware work.

Next: Stage 13.10E Bluetooth discovery/scanning and controller capability discovery above the accepted HCI initialization boundary.


## Roadmap Stage 13.10E — Bluetooth discovery and controller capability contract accepted on 2026-09-30

Stage 13.10E adds bounded BR/EDR GIAC Inquiry discovery above the accepted HCI initialization boundary. It constructs the Inquiry command, parses Inquiry Result and Inquiry Complete events, retains up to 16 unique devices by Bluetooth address, and records page-scan repetition mode, class-of-device, and clock offset. Malformed/truncated events fail closed and duplicate addresses do not consume additional discovery slots.

The HCI layer also parses controller identity/capability responses for Read Local Version, Read BD_ADDR, and Read Local Supported Commands. Opcode, controller status, and response length are validated before exposing HCI/LMP version data, manufacturer/subversion, local Bluetooth address, or the 64-byte supported-command bitmap.

GitHub Actions run 36753355665 passed at exact source SHA `98020bfbfa71dd68f0fc54b234761490947fe346`, including the Stage 13.10E 1/2/4-core acceptance gate and evidence preservation. Release-validation artifact digest: `sha256:dd94f81eed0b3916135051b78c0c27944a33084fba8405c1045dcf49e8196fe2`.

This remains deterministic software validation; physical Bluetooth-radio scanning is not claimed. BLE advertising reports, remote-name discovery, connection establishment, L2CAP, pairing/security, ATT/GATT, lifecycle/hotplug, and physical interoperability remain later work.

Next: Stage 13.10F Bluetooth connection establishment and bounded link lifecycle above the accepted discovery state.


## Roadmap Stage 13.10F — Bluetooth ACL connection and link lifecycle accepted on 2026-09-30

Stage 13.10F consumes bounded discovery records to construct classic BR/EDR Create Connection commands, validates Connection Complete and Disconnection Complete events, and owns up to eight live ACL links by 12-bit controller handle and Bluetooth address. Duplicate identical completion is idempotent; conflicting handle/address reuse fails closed.

The HCI layer now frames and parses bounded ACL packets (1024-byte software ceiling), validates handle/packet-boundary/broadcast fields, and permits inbound/outbound ACL data only for currently owned live handles. Disconnect immediately revokes ACL authority, so stale-handle traffic is rejected.

GitHub Actions run 36761870548 passed at exact source SHA `79cecd7b0e3bab5db4426e2f6fed3eaa451c0a2b`, including Stage 13.10F 1/2/4-core acceptance and evidence preservation. Artifact digest: `sha256:878f9d8866f09fd6e202062dbfa0784073ae7dbed5e196eb223387c26b40410c`.

This remains deterministic software validation. Physical controller timing, ACL flow-control credits, asynchronous transport recovery and real peripheral interoperability remain hardware/production qualification work. Next: Stage 13.10G L2CAP framing and bounded channel lifecycle above owned ACL links.


## Roadmap Stage 13.10G — Bluetooth L2CAP accepted on 2026-09-30

Stage 13.10G adds bounded L2CAP Basic Mode framing above the Stage 13.10F owned ACL link. L2CAP length/CID parsing is exact, CID zero is rejected, payload storage is fixed-capacity, and every frame remains anchored to a live ACL handle.

The signaling layer implements bounded Connection Request/Response and Disconnection Request/Response state with dynamic local CIDs beginning at 0x0040 and a fixed eight-channel table. Established channels own the tuple of ACL handle, PSM, local CID and peer CID. Outbound data targets only the peer CID; inbound data is accepted only for the owned local CID on the same live ACL handle. Teardown revokes channel authority immediately.

GitHub Actions run 36785295506 passed at exact source SHA `c2dff3c12c9c9387a10493900b3846489e8539a6`, including the Stage 13.10G 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:6ac9ff255444ffa36128cd03d75f3d2c89eff086e308618ed21fed463c4a8eab`.

This is deterministic software acceptance. Configuration negotiation, fragmentation/reassembly, enhanced modes, controller flow control and physical interoperability remain outside this boundary. Next: Stage 13.10H pairing, authentication and encryption/security.


## Roadmap Stage 13.10H — Bluetooth pairing/authentication/encryption accepted on 2026-10-01

Stage 13.10H adds a fail-closed BR/EDR security boundary above the accepted ACL/L2CAP ownership layers. A live ACL link progresses explicitly through unauthenticated, authenticating, authenticated, encryption-enabling and secured controller states. Authentication or encryption failure does not grant secured authority.

The stage also adds bounded link-key ownership, Link Key Request positive/negative replies, Link Key Notification ingestion, explicit key revocation, and policy-mediated Secure Simple Pairing interactions for IO capability, numeric confirmation and passkey requests. Pairing confirmation is never silently auto-approved.

Trusted secured authority requires all of: a live owned ACL handle, the same peer address owning a stored link key, successful controller authentication, and controller-confirmed encryption. Removing the key or disconnecting the ACL link makes the trust predicate fail closed.

GitHub Actions run 36791065311 passed at exact source SHA `dda2933eef36026ab70cefbe85898bcd62256f06`, including the Stage 13.10H Bluetooth security 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:f731dbd2f52dc7c2743aa052eaa7f801eef99dc0fc6050f13e6fa71518a07405`.

This is deterministic software acceptance. Persistent secure key storage, full controller event orchestration, physical-radio pairing/interoperability and BLE security remain outside this boundary.


## Roadmap Stage 13.10I — Bluetooth LE foundation accepted on 2026-10-01

Stage 13.10I establishes a bounded Bluetooth Low Energy controller/discovery and connection-lifecycle foundation without conflating LE identity or security with the accepted BR/EDR link-key model. The HCI layer can configure scanning, enable or disable scanning, parse LE Meta Advertising Reports, retain public/random peer address type, capture bounded legacy advertising data and RSSI, and deduplicate repeated reports by address type plus address.

The connection layer consumes an LE discovery record to build LE Create Connection, accepts exact LE Connection Complete events into a fixed eight-link ownership table, rejects malformed, conflicting or out-of-range handle/address state, and removes ownership on Disconnection Complete. A disconnected LE handle is immediately stale and cannot retain authority.

GitHub Actions run 36810460362 passed at exact source SHA `edcfe31b5cd1406c4feb2d001dbc0e2d2f6d7a9f`, including the Stage 13.10I Bluetooth LE foundation 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:f14abafba66e1a599c2410e7e458c3cd2447ca783780deb688afffbfb26b5251`.

This is deterministic software acceptance. Physical LE controller/radio interoperability, extended advertising, LE privacy/resolving lists, connection-update orchestration, ATT/GATT and BLE SMP/bonding remain outside this boundary. Next: Stage 13.10J ATT/GATT foundation above owned LE ACL links.


## Roadmap Stage 13.10J — Bluetooth ATT/GATT foundation accepted on 2026-10-01

Stage 13.10J builds a bounded ATT/GATT software contract above live Stage 13.10I LE link ownership. ATT read/write transactions reject stale LE handles, malformed PDUs, invalid attribute handles and permission violations. The attribute database is fixed-capacity and uses nonzero handles with bounded values.

The GATT layer represents 16-bit Primary Service and Characteristic Declaration/value attributes, preserves ATT permission enforcement, supports bounded service/characteristic discovery by handle range, and models per-LE-link notification/indication subscriptions. Subscription emission rechecks the live LE handle and exact characteristic value handle; unsubscribe or disconnect revokes authority.

GitHub Actions run 36815325948 passed at exact implementation SHA `48bdd085e8d8918bb058a59de06d42ce8ad7108a`, including the Stage 13.10J 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:9247004a999b8470e5431a2396d1d1237bd1cd278f43abdf9ebf9460ed0a3dca`.

This is deterministic software acceptance. Physical BLE ATT/GATT interoperability, ATT MTU negotiation, 128-bit UUIDs, full wire-level discovery procedures, indication confirmations, BLE SMP/bonding and protected persistent keys remain outside this boundary. Next: Stage 13.10K BLE profiles and services above the accepted ATT/GATT authority model.


## Roadmap Stage 13.10K — BLE profiles and services acceptance candidate on 2026-10-01

Stage 13.10K builds bounded BLE services above the accepted Stage 13.10J ATT/GATT authority model. Standard profile support includes Device Information with read-only Manufacturer Name and Model Number characteristics plus Battery Service with a range-checked read-only Battery Level.

Battery updates can emit Handle Value Notifications only through an existing per-link, per-value-handle GATT subscription. Failed authorization, invalid levels, wrong handles or stale LE links leave the prior battery state unchanged. The stage also adds a bounded WovenHat OS service with a read-only status characteristic and write-only command characteristic, preserving ATT permission checks and live-link authority.

The accepted profile checkpoint run 36819330823 passed at exact source SHA `c5f3c6194c58ad16e75c0f44cf90c6d336393c40`. Preserved evidence digest: `sha256:677f88e58b878de73553225ec533f76c4d1b0e74864b1a69aaa30367caef50db`.

Closure hardening makes characteristic creation transactional with respect to ATT table capacity: two slots are preflighted before declaration/value insertion, preventing a capacity failure from leaving an orphan declaration.

This remains deterministic software validation. Physical BLE profile interoperability, standardized conformance testing, BLE SMP/bonding, persistent protected keys and production controller recovery remain outside this boundary. Next after final exact-head acceptance: Stage 13.10L Bluetooth lifecycle and hardening.


## Roadmap Stage 13.10L — Bluetooth lifecycle and hardening acceptance candidate on 2026-10-01

Stage 13.10L hardens the accepted BLE software authority model around teardown, reset and reconnect. `BluetoothLeLifecycle` coordinates live `LeLinkState` with `GattSubscriptions`: a valid disconnect removes the link and revokes every subscription owned by that handle, while controller reset clears all LE links and subscriptions. A monotonic wrapping generation records successful lifecycle invalidation.

Malformed disconnect events and unknown/stale handles fail closed without changing links, subscriptions or generation. Reconnecting a peer does not inherit its previous subscription authority; explicit re-subscription is required before notification emission is authorized.

The bounded stress gate executes 32 connect/subscribe/notify/teardown cycles, alternating normal disconnect with controller reset. Every cycle must end with zero live links, zero subscriptions and stale notification rejection. The accepted stress checkpoint run 36825376710 passed at exact SHA `4c0b0e5e0c8d3b9e00b47dc58868a154c9c970ef`; preserved evidence digest `sha256:67cc6f35203061ef8a79dfba3a8fef739db667bf7fb907b1eb989659c104f9e0`.

This acceptance remains deterministic software validation. Physical Bluetooth controller/radio recovery, transport fault injection, BLE SMP/bonding/LTK security, persistent protected keys, radio interoperability and conformance certification remain outside this boundary.


## Roadmap Stage 13.10R — BLE secure-session identity binding accepted on 2026-10-01

Protected LE ATT/GATT operations now require the transient secure session to
match the live link's peer address type and address as well as its connection
handle. This rejects stale authorization when a controller reuses a handle for
a different peer, including outbound notifications and indications. The exact
tip passed host tests, warning-denying kernel Clippy, and the Stage 13.10
1/2/4-CPU QEMU gate with the identity-binding marker.

This remains deterministic software validation. Physical BLE controller/radio
interoperability, transport fault injection, BLE SMP/bonding/LTK security,
persistent protected keys, radio interoperability and conformance
certification remain outside this boundary.


## Stage 13.11A — BLE SMP pairing/fixed-channel foundation — 2026-10-05

The BLE Security Manager Protocol foundation validates bounded Pairing Request
and Response parameters, enforces supported key-size and distribution flags,
and accepts SMP frames only on fixed L2CAP CID `0x0006` for a live LE link.
Pairing negotiation is handle-bound and fail-closed on malformed or stale
traffic. The dedicated 1/2/4-CPU QEMU gate passes locally; GitHub acceptance
for the pushed tip remains required.

Cryptographic key derivation, user interaction, passkey/numeric comparison,
bonding persistence, controller transport and physical BLE qualification remain
outside this foundation.


## Stage 13.11B — BLE SMP LTK/key distribution — 2026-10-05

Legacy Encryption Information and Master Identification PDUs are now validated
and integrated with the canonical LE bond store. Distribution is bound to the
live pairing handle, negotiated key size and peer identity; malformed, stale,
duplicate and mismatched inputs fail closed. The dedicated 1/2/4-CPU QEMU
gate passes locally; GitHub acceptance for the pushed tip remains required.

Cryptographic derivation, user-confirmed pairing, controller transport and
physical BLE qualification remain outside this bounded distribution stage.

The distribution closure additionally binds pending LTK material to peer
identity, rejects duplicate pending material, and rejects completion after
connection-handle reuse.


## Stage 13.11C — BLE SMP confirm/random verification — 2026-10-05

Pairing Confirm and Pairing Random verification is now handle-bound and tied to
the negotiated pairing parameters and peer address context. Confirm mismatches
transition the bounded state to failure; malformed or out-of-order messages
fail closed. The dedicated 1/2/4-CPU QEMU gate passes locally; GitHub
acceptance for the pushed tip remains required.

This is a protocol/crypto boundary, not complete production pairing. User
confirmation UX, authenticated key-derivation integration, bonding
persistence, controller transport and physical BLE qualification remain open.


## Stage 13.11F — BLE controller encryption/session authority — 2026-10-05

The SMP flow now owns the LE Start Encryption command boundary and binds
Encryption Change completion to the pending handle, peer identity, key size and
authentication state. Failed, disabled, stale or mismatched events fail closed
and revoke authority. The dedicated 1/2/4-CPU gate passes locally; GitHub
acceptance for the pushed tip remains required.

Physical controller transport, SMP interoperability, bond persistence and
hardware qualification remain outside this deterministic software boundary.


## Stage 13.11G — BLE identity/privacy foundation — 2026-10-05

The SMP layer now validates identity distribution, stores bounded IRK-backed
peer identities, and resolves private addresses only against retained identity
material. Duplicate, malformed and stale inputs fail closed, while pending LTK
distribution remains bound to the live peer identity. The dedicated 1/2/4-CPU
gate passes locally; GitHub acceptance for the pushed tip remains required.

Physical privacy-list behavior, SMP interoperability, persistent protected keys
and hardware qualification remain outside this deterministic foundation.


## Stage 13.10S closure hardening — 2026-10-05

Minimum BLE encryption-key-size policy is enforced for both protected ATT/GATT
requests and outbound notifications/indications. Weak authenticated sessions
are rejected before either inbound data access or outbound value encoding;
sessions meeting the configured threshold remain bound to the live peer
identity. The exact 1/2/4-CPU Stage 13.10 gate, host tests and warning-denying
kernel Clippy passed locally. GitHub acceptance for the source tip is required
before this closure is considered complete.


## Stage 13.10T — outbound BLE key-size acceptance contract — 2026-10-05

The Stage 13.10 runtime harness now requires a dedicated marker for outbound
notification authorization by minimum encryption key size. A weak authenticated
session is rejected before notification encoding, while a session meeting the
16-byte policy is accepted. The local 1/2/4-CPU gate, host tests and
warning-denying kernel Clippy pass; GitHub acceptance for the pushed tip is
still required.

## Stage 1–5 WDJ2 capacity extension — 2026-10-14

The compatible WDJ2 batch journal now accepts the full 255-entry range
representable by its existing one-byte count field, with a host round-trip
self-test at the maximum. This removes the former sixteen-file implementation
ceiling for package/directory transactions without changing the on-disk
version or production block-device ABI. Recovery allocation remains explicitly
bounded; arbitrary-size/full-volume rollback, physical storage/DMA, torn-write
and reboot qualification remain open.

The public `persist_paths_atomic` boundary now returns `TooLarge` for a
256-entry request before mount or device access, rather than collapsing the
bounded-capacity refusal into a generic failure.



