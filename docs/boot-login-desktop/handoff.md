# Handoff

Phase A: COMPLETE (inventory and baseline only). Runtime source a980619e.
Branch: test/boot-login-baseline. Integration source: d21807ba; local merge
aeb9d40a. Repairs: 6b4cf263 diagnostics/probe, b75b9366 persistent test fixture,
a980619e mixed-edition formatting. Full directive is archived in directive.md.

Passed: full RUN-STAGE10.7.ps1, including 60 memory repetitions and 1/2/4-CPU
network/Stage10.7; 208 Python tests (six skips); host Rust tests; formatting;
host/default and affected feature Clippy; Stage12.1–12.5 QEMU 1/2/4 CPUs.
Evidence: audit-artifacts/acceptance-20261104-194935-034/ and
boot-login-baseline-repair-20261104/. Earlier failures and build/network
restriction diagnoses remain preserved. No ABI, capability, unsafe code or
new kernel object was added. No release, physical-media or firmware operation
was performed. No complete boot/login/desktop milestone is claimed.

Exact next unit: Phase B1, reject malformed/trailing-data boot configuration
before kernel handoff, using the existing version-matched UEFI implementation.
Cache source was compared against its package archive; provenance is retained.
Claim vendor/bootloader build integration and the UEFI/config test paths before
editing. Do not modify the Cargo registry cache. Preserve accepted BootInfo,
valid legacy boot configuration and all successful boot behavior. Full versioned
boot policy and trust-chain qualification remain separate requirements.

Only one Cargo/acceptance chain may run at once: separate worktrees still share
build artifacts. No runtime chain is active at this Phase A checkpoint.