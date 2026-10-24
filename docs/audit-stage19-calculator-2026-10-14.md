# Stage 19 WovenCalculator application audit — 2026-10-14

WovenCalculator is the first roadmap Stage 19 system application delivered as
a separate crate. It is not linked into the kernel and uses `libwoven` for all
userspace operations.

## Safety and ABI boundary

- The expression evaluator is allocation-free, bounded to 96 input bytes and
  16 parenthesis levels, and uses checked signed 64-bit arithmetic.
- Empty, malformed, unbalanced, divide-by-zero, overlong, and overflowing
  expressions return typed errors; no malformed input path panics.
- The binary is a static ET_EXEC image linked with the same large-code-model,
  W^X, no-RELRO policy as the existing external applications.
- Release packaging uses the existing WAPP signer. The GUI mode requests only
  the normal display syscalls and has no kernel-side privilege bypass.

## Evidence

- Three host parser tests passed.
- Host library Clippy and freestanding binary Clippy passed with `-D warnings`.
- Signed WAPP packaging succeeded.
- `scripts/test-calculator-qemu.py` passed on 1/2/4 CPUs (exit 33):
  `audit-artifacts/woven-calculator-1cpu`,
  `audit-artifacts/woven-calculator-2cpu`, and
  `audit-artifacts/woven-calculator-4cpu`.

The QEMU application test proves package verification, filesystem round-trip,
ELF loading, Ring-3 execution, and clean zero exit using the existing Stage
16.1 acceptance boundary.

## Scope remaining

This is one application, not completion of the roadmap's full WovenFiles,
WovenTerminal, WovenSettings, WovenMonitor, WovenSecurity, WovenEditor,
WovenImages, WovenUpdate, and WovenStore suite. Catalog installation into a
boot image and a full GUI toolkit remain later integration work.
