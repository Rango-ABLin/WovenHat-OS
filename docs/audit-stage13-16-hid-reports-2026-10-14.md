# Stage 13.16 report-protocol HID input audit — 2026-10-14

## What changed

**`kernel/src/hid.rs` (pure; host-tested).**
- `parse` reads a report descriptor into a bounded `Layout`: the top-level
  application and up to 48 input fields (usage, report ID, bit offset and
  size, logical range, relative).
- It supports report IDs, usage ranges, push and pop, constant padding and
  long items. It refuses malformed descriptors, descriptors that are too
  large, and applications it does not decode (keyboards stay on the boot
  path).
- `Decoder` reports changes only:
  - relative and absolute pointer motion, wheel and button edges;
  - pen position, pressure and contact (hover included, nothing out of
    range);
  - per-contact touch, with each lift reported once;
  - game-controller axes, hat and buttons.
- Positions are scaled to 0..=32767, pressure to 0..=1023, and controller
  axes to -32768..=32767.

**xHCI.** A HID interface that is not a boot keyboard or mouse has its
report descriptor fetched (its length comes from the HID class descriptor)
and parsed. Only a decodable layout becomes an input device, with reports
up to 64 bytes. The worker keeps one `Decoder` per report device and
publishes outside the controller lock.

**WovenInput and the userspace ABI.**
- `Scroll`, `PointerAbsolute`, `Touch`, `Pen` and `GameController` are now
  production events. Before this stage they existed only in the Stage 13.7
  probe build.
- The input syscall encodes them as kinds 9–13, and libwoven's `InputEvent`
  decodes them.

## Gates

**Host tests (`tests/hid_reports.rs`), against real descriptors.**
- QEMU's usb-tablet: absolute position, wheel with sign extension, and
  button edges.
- A 14-button game pad: hat null state, four axes, buttons, and changes
  only.
- A digitizer pen with report ID 2: position, pressure, hover, out of range,
  and foreign report IDs.
- A two-contact touch screen: per-contact reports and lifts.
- Malformed, unbalanced, keyboard-only and zero-size descriptors, and
  short reports.

The game-pad test caught a real bug: a fully deflected axis scales to
`i16::MIN`, which had been used as the "no value yet" marker, so it was
never reported. The marker is now `Option<i16>`.

**QEMU: `[S13.16] report-protocol HID input: PASSED`.** Harness:
`scripts/test-stage13-16-hid-reports.py`.
- The machine is q35 with QEMU's xHCI and a USB tablet.
- Bring-up must parse the tablet's report descriptor as a pointer device.
- QMP then injects absolute (8192, 24576) and (24576, 8192), a left press,
  a move while the button is held, a release, and a wheel step up and down.
- They must arrive in order:
  - the positions exactly, X before Y;
  - one press and one release;
  - `Scroll` +1 and then -1;
  - all of it carried by device interrupts.

## Evidence (`audit-artifacts/`)

Stage 13.16 on 1/2/4 CPUs, all passing, with the held-button injection:
`stage13.16-1cpu-1792009812938824900`, `stage13.16-2cpu-1792009909416253200`, `stage13.16-4cpu-1792009987932861000`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. All four built, and each failed the gate as intended:
- **Report descriptor never fetched:** the tablet is not an input device.
- **X and Y swapped:** the positions arrive mirrored and out of order.
- **Held buttons reported again as presses:** the click check fails.
  - The first run of this control was not caught: the injection never
    produced a report while a button was held.
  - The harness now moves the pointer during the press, and the control is
    caught.
- **Wheel not sign-extended:** the step down arrives as +127.

After the restore, the gate passes again.

Regression, all passing:
- Stage 13.13 on 1/2/4 CPUs.
- 13.5: `stage13.5-1cpu-1792008823436264500`, `stage13.5-2cpu-1792008905908146600`, `stage13.5-4cpu-1792008969277639000`
- 13.6: `stage13.6-1cpu-1792009030435929400`, `stage13.6-2cpu-1792009113797248000`, `stage13.6-4cpu-1792009183415151900`
- 13.7: `stage13.7-1cpu-1792009248467170400`, `stage13.7-2cpu-1792009339592498600`, `stage13.7-4cpu-1792009411593361400`
- Stage 19.12 (pointer input) on 1/2/4 CPUs, through its own harness.
- 13.12 `devices` and 13.15 on 2 CPUs.

**Lint and tests.** Warning-denying Clippy passes for:
- production;
- `stage13-6-test`, `stage13-7-test`, and `stage13-12-test` through
  `stage13-16-test`;
- `stage19-12-test`;
- host `--all-targets`.

The host HID tests and the libwoven tests pass.

## Limits

- **Single application per device.** A device whose report descriptor has
  several top-level applications (for example a keyboard with a touchpad)
  is decoded for its first decodable application only.
- **Gate coverage.** QEMU provides only the tablet as a report-protocol
  device, so pens, touch screens and game pads are covered by the host
  tests on real descriptors, not by a QEMU gate.
- **No virtio-input.** Touch input through virtio-input (`virtio-tablet`,
  `virtio-multitouch`) needs a virtio-input driver.
- **QEMU only.** Physical digitizers and controllers are not qualified.
