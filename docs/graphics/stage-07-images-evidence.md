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

- `cargo test -p libwoven --quiet`: 157 passed.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- Freestanding kernel Clippy and release build: passed.
- WovenShell freestanding release build: passed.
- Production system-app QEMU at 1 CPU: passed with a 1280x800 guest frame and
  the wallpaper VFS readback marker.
- Production system-app QEMU at 2 CPUs: passed with a 1280x800 guest frame and
  the wallpaper VFS readback marker.

## Gate status

The requested wallpaper deliverables are complete and verified. Stage 07 as a
whole remains partial because WebP pixel decoding is still host-target only;
this increment deliberately does not claim a freestanding WebP backend.
