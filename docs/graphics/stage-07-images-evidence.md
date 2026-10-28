# Graphics roadmap Stage 07 image and asset evidence

Date: 2026-10-25

## Current increment

`libwoven::media` now enforces a 8192-pixel dimension limit and a 16-megapixel
pixel budget, recognizes PNG/GIF/BMP/JPEG and VP8/VP8L/VP8X WebP headers, and provides a
caller-owned uncompressed 24/32-bit BMP-to-RGBA decoder. Offset, stride,
compression, orientation, truncation, and output-capacity checks happen before
pixel writes. The decoder is allocation-free and suitable for a bounded
wallpaper/thumbnail worker.

The codec boundary now also includes pure-Rust PNG and JPEG RGBA adapters,
bounded by the same caller output capacity, plus host and freestanding WebP
RGBA adapters. The freestanding path uses pinned alloc-only `zenwebp 0.3.2`
and decodes lossless VP8L, lossy VP8, and extended VP8X/ALPH images into the
caller-owned RGBA buffer. `ThumbnailCache`
provides fixed round-robin storage, `WallpaperCatalog` requires an SPDX-like
license identifier and nonzero source digest, and `load_vfs_asset` reads one
bounded immutable asset through the normal VFS descriptor API. Its guest read
path is chunked to the kernel's 1024-byte syscall limit, so larger assets do
not silently fail.

The shipped wallpaper is a deterministic WovenHat-original 160x90 BMP24
gradient. It is declared CC0-1.0 in `assets/wallpapers/manifest.toml` with
SHA-256 `7c059d7f598f220879a4f1af16388df047c0b726cd0eaf2b2624317b208f9de9`.
`kernel/build.rs` generates and embeds the exact binary; production app
installation writes it read-only to `/share/wallpapers/wovenhat-gradient.bmp`
and installs the manifest at `/share/wallpapers/manifest.toml`; it reads all
43,254 wallpaper bytes back from the VFS. WovenImages loads that path,
decodes it through `libwoven::media`, and draws a bounded nearest-neighbor
thumbnail through the shared one-entry `ThumbnailCache` in its GUI surface.

## Evidence

- `cargo test -p libwoven --all-targets --quiet`: 160 passed.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- Freestanding kernel Clippy and release build: passed.
- WovenImages freestanding Clippy and kernel Stage 07 Clippy: passed.
- Dedicated signed Ring-3 WebP self-test QEMU passed at 1, 2, and 4 CPUs,
  including the guest `[WEBP] freestanding decode: PASSED 2x2` marker.
- Evidence directories: `audit-artifacts/woven-images-1cpu`,
  `audit-artifacts/woven-images-2cpu`, and `audit-artifacts/woven-images-4cpu`.

## Gate status

Stage 07 image decoding and wallpaper deliverables are accepted for the
freestanding/QEMU scope. Release distribution still needs the documented
`zenwebp 0.3.2` AGPL-3.0-or-later compliance decision or a commercial license;
that is a legal release gate, not an untested codec path.
