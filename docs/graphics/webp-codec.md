# Freestanding WebP codec boundary

Freestanding `libwoven::codecs::decode_webp_rgba` uses the pinned alloc-only
`zenwebp` 0.3.2 decoder. Host builds continue to use the existing MIT-licensed
OxideAV adapter. The decoder writes into caller-owned RGBA storage after the
shared WebP dimension and pixel-budget checks; malformed or truncated input
fails through `ImageError`.

Release licensing requirement: `zenwebp` 0.3.2 is licensed
`AGPL-3.0-or-later`. A production distribution must comply with that license
or obtain an appropriate commercial license before shipping the freestanding
backend. This dependency is deliberately pinned and documented rather than
treated as an unlicensed permissive component.
