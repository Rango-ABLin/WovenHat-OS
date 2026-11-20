# M5 Performance Report

Date: 2026-11-20
Branch: `m5/hardware-qualification` at `5ca5b9b3`

## Measured in this slice

- M5 DMA QEMU gate: 1/2/4 CPUs, 4 GiB guest, PASS (exit 33), elapsed evidence
  under `audit-artifacts/m5-dma-{1,2,4}cpu-*`.
- Storage gates: NVMe 1 CPU PASS, AHCI 1/2/4 CPU PASS.
- Input gate: Stage 13.13 USB input, 1 CPU PASS.

## Notes

- No dedicated latency/Jitter or throughput benchmarks were added in this M5
  slice; the existing gates measure functional completion, not performance
  budgets.
- DMA quarantine/bounds behavior is exercised by host tests and QEMU DMA gate.
- Performance claims in this milestone are limited to gate-level evidence;
  formal performance budgets remain open.

See `docs/milestones/M5_TEST_RESULTS.md` for raw command/evidence lines.
