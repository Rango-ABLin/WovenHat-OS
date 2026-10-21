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
- Full workspace tests and warning-denying Clippy are required before this
  increment is committed.

## Limits

This is not full internationalization. Unicode normalization and property
data, bidi/RTL ordering, script shaping, keyboard layouts, IMEs, translated
application catalogs, and compositor integration remain open prerequisites.
