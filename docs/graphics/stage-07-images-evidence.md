# Graphics roadmap Stage 07 image and asset evidence

Date: 2026-10-25

## Current increment

`libwoven::media` now enforces a 8192-pixel dimension limit and a 16-megapixel
pixel budget, recognizes PNG/GIF/BMP/JPEG and VP8X WebP headers, and provides a
caller-owned uncompressed 24/32-bit BMP-to-RGBA decoder. Offset, stride,
compression, orientation, truncation, and output-capacity checks happen before
pixel writes. The decoder is allocation-free and suitable for a bounded
wallpaper/thumbnail worker.

The codec boundary now also includes pure-Rust PNG and JPEG RGBA adapters,
bounded by the same caller output capacity, plus a host-target WebP RGBA
adapter. The freestanding WebP path fails closed because the current VP8
dependency is not no-std; it is not linked into the kernel. `ThumbnailCache`
provides fixed round-robin storage, `WallpaperCatalog` requires an SPDX-like
license identifier and nonzero source digest, and `load_vfs_asset` reads one
bounded immutable asset through the normal VFS descriptor API.

## Evidence

- `cargo test -p libwoven --quiet`: 157 passed.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- Freestanding kernel Clippy and release build: passed.
- WovenShell freestanding release build: passed.

## Gate status

Stage 07 remains partial. WebP is only host-target complete, and the repository
still lacks licensed wallpaper binaries/manifests, persistent VFS boot-image
installation, thumbnail rendering in WovenImages, and guest full-resolution
image acceptance. This commit must not be treated as the Stage 07 acceptance
gate.
