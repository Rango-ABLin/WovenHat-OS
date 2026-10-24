# Stage 26.5 bounded internationalization foundation audit - 2026-10-21

## Scope

This increment adds `libwoven::i18n`: fixed-size English and Chichewa locale
identifiers, bounded translation catalog insertion/lookup, Gregorian date
validation and locale-aware date ordering, caller-buffer integer formatting,
and Unicode scalar validation.
The follow-up adds a transactional Latin combining-mark normalization subset;
unsupported combining sequences fail closed. The latest follow-up adds a
caller-buffered strong-direction run resolver for common Latin/Hebrew/Arabic
text and numbers; it is explicitly not full UAX#9.

## Verification

- `cargo fmt --all -- --check` passed.
- `cargo test --offline -p libwoven` passed: 109 tests, including nine new
  i18n tests.
- `cargo clippy --offline -p libwoven --all-targets -- -D warnings` passed.
- `cargo test --offline --all` passed: 109 `libwoven` tests and all workspace
  integration suites passed.
- `cargo clippy --offline --all-targets -- -D warnings` passed for the full
  workspace.
- `cargo clippy --offline -p wovenhat-kernel --target x86_64-unknown-none
  --features system-apps -- -D warnings` passed with the generated signed
  application image environment.
- `python scripts/test-shell-qemu.py --system-apps --cpus 1` passed.
- `python scripts/test-shell-qemu.py --system-apps --cpus 2` passed.
- `python scripts/test-shell-qemu.py --system-apps --cpus 4` passed.

The QEMU runs are regression evidence for the production system-app image and
SMP boot path. During this follow-up, the first two 1-CPU attempts reproduced
a user-mode stack write fault at the former 8 KiB shell-stack boundary before
F12 diagnostics could open. The bounded `UserStack::SIZE` was increased to
16 KiB while retaining the guard page; the corrected 1/2/4-CPU runs passed.
This library increment adds no i18n-specific kernel feature gate or kernel
self-test of its own.

## Limits

This is not full internationalization. Complete Unicode normalization and
property data, full bidi/RTL ordering, script shaping, keyboard layouts, IMEs, and
translated application catalogs remain open prerequisites. The WovenShell
lock screen now consumes bounded English/Chichewa catalogs, but this is not
application-wide translation coverage or reviewed localization.
