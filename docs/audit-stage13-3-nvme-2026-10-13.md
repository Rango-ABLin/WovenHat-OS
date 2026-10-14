# Stage 13.3 NVMe audit — 2026-10-13

## Result

The bounded Stage 13.3 NVMe acceptance gate passed on 1, 2, and 4 CPUs after
the controller-failure lifecycle repair:

- 1 CPU: `audit-artifacts/stage13.3-1cpu-1791894897482681000`
- 2 CPUs: `audit-artifacts/stage13.3-2cpu-1791894909297351100`
- 4 CPUs: `audit-artifacts/stage13.3-4cpu-1791894920160288000`
- marker: `=== STAGE 13.3 ACCEPTANCE: PASS ===`

The gate boots the QEMU NVMe fixture, identifies a namespace, creates admin
and I/O queues, writes and reads a disposable sector, and flushes it. The
feature-specific freestanding Clippy gate and `cargo build --features
stage13-3-test` also pass.

## Lifecycle repair

`DmaPage` now owns its physical frame and returns it to the frame allocator on
drop. Initialization records the original PCI command register and installs a
rollback guard immediately after enabling memory decoding and bus mastering.
Every later failure attempts bounded controller disable/readiness polling and
restores the exact PCI command value, preventing a failed probe from leaving
DMA enabled or leaking its queue/identify buffers.

## Acceptance boundary

This is a production-quality emulated NVMe foundation, not a claim of complete
physical NVMe support. The current implementation still uses bounded polling,
one namespace, one synchronous I/O queue, legacy per-access MMIO aliases, and
no interrupt-backed completion, reset/recovery workflow, hotplug removal, or
multi-namespace/multipath policy. Those are explicit prerequisites before a
physical NVMe device can be called production-qualified. The inserted USB
flash disk was not modified; it remains outside this QEMU-only acceptance.
