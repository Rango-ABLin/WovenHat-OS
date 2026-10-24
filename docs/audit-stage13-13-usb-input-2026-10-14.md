# Stage 13.13 USB keyboards and mice audit — 2026-10-14

## What changed

**xHCI input (`kernel/src/xhci.rs`).** `start_input` runs from bring-up
once `xhci0` is ready.
- **Enumeration.** It enumerates every connected root port, each with its
  own reset and slot. Each keyboard or mouse is kept as an `InputDevice`
  with its own slot, EP0 ring, contexts, interrupt ring and report buffer,
  up to four devices.
- **Report mode.** Each device is switched to report-on-change
  (`SET_IDLE 0`) and has one interrupt-IN transfer armed.
- **Interrupt binding.** An MSI-X or MSI vector is bound through the
  driver manager, the `usb-input` worker is registered as the vector's
  waiter, and the interrupter is enabled with 1 ms moderation.

**The worker.** Under the controller lock it acknowledges IMAN, drains the
event ring without waiting, copies each report and re-arms its transfer.
After unlocking it decodes and publishes into WovenInput: key-down edges
with shift (held keys do not repeat, rollover reports are ignored), and
mouse motion and button edges. It then sleeps in `wait_for_event`.

**Interrupt wakeups (`kernel/src/interrupts.rs`).** Each PCI vector
(0x90–0xcf) can have one waiter task. The generic handler signals it
through the scheduler event latch after publishing the work flag.

**MSI-X delivery fix (`kernel/src/hal/pci/msix.rs`).** `enable_owned`
asserted Function Mask while programming the table entry and never cleared
it. With Function Mask set, every vector is masked, so no MSI-X interrupt
was ever delivered on any device. The Stage 13.2 gate checked programming,
not delivery. Activation now clears Function Mask, and verifies the write,
after unmasking the entry. A failure there keeps the lease for ordered
teardown, like the other late activation failures.

**Mouse axis fix.** `decode_boot_mouse` negated Y. HID boot mice report Y
positive towards the user (down the screen), which is WovenInput's
orientation. The decoder and its self-test are corrected, and the Stage
13.13 gate now checks the axis with real injected motion.

## Gate

`[S13.13] USB input: PASSED`. Harness:
`scripts/test-stage13-13-usb-input.py`.
- **Machine:** q35 with QEMU's xHCI, a USB keyboard and a USB mouse.
- **Decoder self-test:** presses arrive once, held keys do not repeat,
  shift applies, and rollover reports are ignored.
- **Discovery:** bring-up reports xhci0 Ready, with one keyboard and one
  mouse on a PCI device vector.
- **Injection:** after the kernel's ready line, the harness sends QEMU
  input over QMP, on a free local port. QEMU routes it to the USB devices,
  which were added after the chipset PS/2 devices:
  - typing `w`, `h`, `1`, shift+`a`, Enter must arrive as exactly `'w' 'h'
    '1' 'A' Enter`;
  - moving right 40, down 25, then a left click must arrive as motion
    (40, 25), a press and a release.
- **Delivery:**
  - the vector's interrupt count rises and at least five USB reports are
    copied, so input that took the PS/2 path would fail;
  - worker passes are at most interrupts + 2, so the worker sleeps between
    interrupts rather than polling.

In the 1-CPU run, 15 interrupts carried 15 reports in 17 worker passes.

## Evidence (`audit-artifacts/`)

Stage 13.13 on 1/2/4 CPUs, all passing: `stage13.13-1cpu-1791997347904187000`, `stage13.13-2cpu-1791996407064891200`, `stage13.13-4cpu-1791996488426811900`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. Each one failed the gate as intended:
- **Transfer not re-armed:** only the first report per device arrives
  (`w`, and the first motion).
- **Function Mask left set** (the original bug): nothing arrives.
- **Worker polls instead of sleeping:** every event arrives, but the gate
  fails at "the worker sleeps between them".
- **Held keys repeat:** the decoder check fails.
- **Mouse Y inverted again:** motion arrives as (40, −25).
- **No input injected:** nothing arrives.

After the restore, the gate passes again on 1 CPU.

Regression on 1/2/4 CPUs, all passing:
- 13.12 `devices`: `stage13.12-devices-1cpu-1791997427047816800`, `stage13.12-devices-2cpu-1791997514470681600`, `stage13.12-devices-4cpu-1791997591909102600`
- 13.12 `nodisk`: `stage13.12-nodisk-1cpu-1791997665910623600`, `stage13.12-nodisk-2cpu-1791997736519754300`, `stage13.12-nodisk-4cpu-1791997807852208900`
- 13.12 `absent`: `stage13.12-absent-1cpu-1791997878042457400`, `stage13.12-absent-2cpu-1791997944166498300`, `stage13.12-absent-4cpu-1791998009777679000`
- 13.12 `production`: `stage13.12-production-1cpu-1791998078111124800`, `stage13.12-production-2cpu-1791998128345580800`, `stage13.12-production-4cpu-1791998163544483500`
- 13.2: `stage13.2-1cpu-1791998197713407000`, `stage13.2-2cpu-1791998288138623900`, `stage13.2-4cpu-1791998348411565000`
- 13.5: `stage13.5-1cpu-1791998399483751100`, `stage13.5-2cpu-1791998488400660100`, `stage13.5-4cpu-1791998553506569200`
- 13.6: `stage13.6-1cpu-1791998614369898400`, `stage13.6-2cpu-1791998697837326700`, `stage13.6-4cpu-1791998767761400400`
- 13.7: `stage13.7-1cpu-1791998831173952000`, `stage13.7-2cpu-1791998909034208900`, `stage13.7-4cpu-1791998971158633900`
- 13.9: `stage13.9-1cpu-1791999032502021800`, `stage13.9-2cpu-1791999133577684100`, `stage13.9-4cpu-1791999212910642800`
- 13.10: `stage13.10-1cpu-1791999292801794200`, `stage13.10-2cpu-1791999362484614300`, `stage13.10-4cpu-1791999416399032400`

**Lint.** Warning-denying Clippy passes for:
- production;
- `stage13-13-test` and `stage13-12-test`;
- `stage13-2-test`, `stage13-5-test`, `stage13-6-test`, `stage13-7-test`,
  `stage13-9-test` and `stage13-10-test`;
- `stage14-5d-test`;
- host `--all-targets`.

## Limits

- **Boot protocol only.** Report-protocol HID devices such as touchpads
  and multi-button mice need the report descriptor parser; that work is
  part of Stage 13.16.
- **No hotplug.** Devices are enumerated at bring-up only. Port Status
  Change events are drained but not acted on.
- **Limited keymap.** The keyboard map covers the keys WovenInput can
  represent today: printable US layout, Enter, Backspace, Tab and F1.
- **QEMU only.** Physical USB controllers and devices are not qualified.
