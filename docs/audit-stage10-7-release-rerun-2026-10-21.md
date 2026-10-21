# Stage 10.7 release-chain revalidation — 2026-10-21

## Evidence

The authoritative `RUN-STAGE10.7.ps1` chain completed its host regressions,
warning-denying host Clippy, host tests, Stage 7.5/7.6 preservation loops,
1/2/4-CPU networking, and preserved Stage 8.1 through Stage 10.6 markers.

The chain's first isolated Stage 10.7 image build failed with Rust/LLVM
`no space on device` after the build volume reached zero free space. This was
an infrastructure failure; no test assertion or source diagnostic failed.

After task-owned QEMU cleanup and removal of disposable `.cargo-build`
output, the isolated TCP gate was rerun directly:

- `python scripts/test-stage10-7-tcp.py --cpus 1`: PASS
- `python scripts/test-stage10-7-tcp.py --cpus 2`: PASS
- `python scripts/test-stage10-7-tcp.py --cpus 4`: PASS

Each run verified two host-to-guest TCP transmissions, guest response, and
remote FIN/EOF completion. Serial evidence is preserved in
`target/stage10-7-regression-{1,2,4}-debug/serial.log`.

## Scope and limits

This revalidation made no source changes and does not replace the physical
TPM, USB, or real-hardware qualification gates. The shared agent's unrelated
uncommitted persistent-account provisioning edit was preserved.
