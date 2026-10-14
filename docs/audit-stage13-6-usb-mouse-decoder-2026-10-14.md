# Stage 13.6 USB HID mouse report decoder — 2026-10-14

`xhci::decode_boot_mouse` decodes the fixed USB HID boot-protocol report
without borrowing DMA memory or allocating. It bounds the report to the
three button bits, converts signed X/Y deltas to the WovenInput coordinate
convention, emits at most one movement event plus three button-edge events,
and returns the new button state for the next report. Publication remains the
caller's responsibility after releasing the controller lock.

The decoder's motion, press, and release paths are included in the xHCI
self-test. Host Clippy, freestanding Stage 13.6 Clippy, all host tests, and
the Stage 13.6 QEMU gate passed on 1/2/4 CPUs. Evidence:

- `stage13.6-1cpu-1791982285425284200`
- `stage13.6-2cpu-1791982343461199100`
- `stage13.6-4cpu-1791982390647822300`

This closes report decoding, not the whole device path. A production xHCI
input worker still needs to poll/submit mouse reports and publish them into
WovenInput, and native USB mouse interoperability remains hardware-dependent.
