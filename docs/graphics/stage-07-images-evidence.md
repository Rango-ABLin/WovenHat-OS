# Graphics roadmap Stage 07 image and asset evidence

Date: 2026-10-25

## Current increment

`libwoven::media` now enforces a 8192-pixel dimension limit and a 16-megapixel
pixel budget, recognizes PNG/GIF/BMP/JPEG and VP8X WebP headers, and provides a
caller-owned uncompressed 24/32-bit BMP-to-RGBA decoder. Offset, stride,
compression, orientation, truncation, and output-capacity checks happen before
pixel writes. The decoder is allocation-free and suitable for a bounded
wallpaper/thumbnail worker.

## Evidence

- `cargo test -p libwoven --quiet`: 154 passed.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.

## Gate status

Stage 07 remains partial. PNG/JPEG/WebP pixel decoders, a persistent asset
manifest/cache, licensed wallpaper fixtures, VFS boot-image integration, and
guest full-resolution image acceptance are not yet implemented. This commit
must not be treated as the Stage 07 acceptance gate.
