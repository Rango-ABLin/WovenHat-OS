# Stage 26.5 bounded internationalization foundation audit - 2026-10-21

## Scope

This increment adds `libwoven::i18n`: fixed-size English and Chichewa locale
identifiers, bounded translation catalog insertion/lookup, Gregorian date
validation and locale-aware date ordering, caller-buffer integer formatting,
and Unicode scalar validation.

## Verification

- `cargo fmt --all -- --check` passed.
- `cargo test --offline -p libwoven` passed, including five new i18n tests.
- `cargo clippy --offline -p libwoven --all-targets -- -D warnings` passed.
- `cargo test --offline --all` passed: 105 `libwoven` tests and all workspace
  integration suites passed.
- `cargo clippy --offline --all-targets -- -D warnings` passed for the full
  workspace.
- `python scripts/test-shell-qemu.py --system-apps --cpus 1` passed.
- `python scripts/test-shell-qemu.py --system-apps --cpus 2` passed.
- `python scripts/test-shell-qemu.py --system-apps --cpus 4` passed.

The QEMU runs are regression evidence for the production system-app image and
SMP boot path. This library-only increment adds no kernel feature gate or
kernel self-test of its own.

## Limits

This is not full internationalization. Unicode normalization and property
data, bidi/RTL ordering, script shaping, keyboard layouts, IMEs, translated
application catalogs, and compositor integration remain open prerequisites.
