# Stage 31 supported SMP scalability audit — 2026-10-22

The bounded Stage 31 supported-suite matrix was executed after the 16-CPU
control-plane and capacity change. The five existing suites passed at every
requested profile: 1, 2, 4, 8, and 16 CPUs (25 runs total, exit 33).

The 8/16 evidence includes:

- `target/memory-regression-8-debug/serial.log`
- `target/memory-regression-16-debug/serial.log`
- `target/storage-regression-8-debug/serial.log`
- `target/storage-regression-16-debug/serial.log`
- `target/network-regression-8-debug/serial.log`
- `target/network-regression-16-debug/serial.log`
- `audit-artifacts/woven-security-8cpu/`
- `audit-artifacts/woven-security-16cpu/`
- `target/shell-regression/diagnostic-8cpu-1792672468010623100/serial.log`
- `target/shell-regression/diagnostic-16cpu-1792673058433796700/serial.log`

Representative commands were:

```text
python scripts/test-memory-qemu.py --cpus 8 --timeout 300
python scripts/test-memory-qemu.py --cpus 16 --timeout 300
python scripts/test-storage-qemu.py --cpus 8 --timeout 300
python scripts/test-storage-qemu.py --cpus 16 --timeout 900
python scripts/test-network-qemu.py --cpus 8 --timeout 300
python scripts/test-network-qemu.py --cpus 16 --timeout 900
python scripts/test-security-qemu.py --cpus 8 --timeout 300
python scripts/test-security-qemu.py --cpus 16 --timeout 900
python scripts/test-shell-qemu.py --cpus 8
python scripts/test-shell-qemu.py --cpus 16
```

The first 16-CPU storage run exceeded 300 seconds after making progress and
was retained as `target/storage-regression-16-debug/serial.log`; the rerun
with 900 seconds passed. The first 16-CPU shell run exposed fixed kernel
preemption-test budgets and was retained under the corresponding earlier
`diagnostic-16cpu-*` directory. Scaling those diagnostic budgets, without
removing assertions or reducing workload rounds, produced the passing rerun.

Post-change checks also passed:

- freestanding kernel Clippy with `--no-default-features -- -D warnings`;
- host `cargo clippy --all-targets -- -D warnings`;
- `cargo test -p libwoven` (119 tests passed).

This audit accepts the supported SMP matrix, not the complete Stage 31 torture
or production-readiness milestone. Randomized fault injection, low-resource
and loss recovery, service/driver restart, sudden-shutdown recovery,
repeated-boot acceptance, and native hardware qualification remain open.
