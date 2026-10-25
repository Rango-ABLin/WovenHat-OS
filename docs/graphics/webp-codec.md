# Freestanding WebP codec boundary

Freestanding `libwoven::codecs::decode_webp_rgba` uses the pinned alloc-only
`zenwebp` 0.3.2 decoder. Host builds continue to use the existing MIT-licensed
OxideAV adapter. The decoder writes into caller-owned RGBA storage after the
shared WebP dimension and pixel-budget checks; malformed or truncated input
fails through `ImageError`.

`decode_webp_rgba_stream_bounded` accepts caller-owned encoded workspace and
caller-owned RGBA output. The standalone `tools/webp-allocation-cert` harness
counts the whole host process allocator during VP8L, VP8X/ALPH, and malformed
decodes and enforces a 1 MiB peak-live budget; the verified peak is 702,669
bytes with no leak. This certifies the exercised pinned paths, while the
backend's documented caveat about allocations outside its configured limit
remains an explicit scope boundary.

Release licensing requirement: `zenwebp` 0.3.2 is licensed
`AGPL-3.0-or-later`. A production distribution must comply with that license
or obtain an appropriate commercial license before shipping the freestanding
backend. This dependency is deliberately pinned and documented rather than
treated as an unlicensed permissive component.
