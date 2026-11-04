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

Current branch: feature/boot-failure-validation, based on 51dd9061. B1 runtime
aaeaaf8f is accepted: 27 UEFI configuration cases, signed-handoff 1/2/4 CPUs,
and full Windows preservation audit-artifacts/acceptance-20261104-205802-369/.

Accepted unit: B2 bounded kernel-image checks before handoff at 72be85aa. The new safe Rust
validator checks raw file size, ELF64 x86_64 little-endian headers, program and
section table ranges, load segment bounds/alignment, executable entry, and the
embedded bootloader contract. UEFI uses the existing common mapper and BootInfo.
This is structural validation, not image authentication or a complete ELF
security audit. Host tests and root/UEFI warning-denying Clippy pass; all 45 runtime
matrix cases, full Windows preservation and signed-handoff at 1/2/4 CPUs pass.
Evidence: audit-artifacts/boot-image-b2/ and acceptance-20261104-214457-697/.
Only bounded B2 is accepted. Full Phase B remains open: remaining boot policy,
I/O/allocation/graphics/ExitBootServices failure qualification and trust gates.
Next unit: explicit firmware I/O/resource failure reporting using existing APIs.

Only one Cargo/acceptance chain may run at once: separate worktrees still share
build artifacts. Preserve every historical artifact and unrelated worktree.
