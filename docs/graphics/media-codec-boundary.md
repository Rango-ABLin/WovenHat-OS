# Media and codec boundary

The current media increment adds `libwoven::media::parse_image_header`, a
bounded metadata parser for PNG, GIF, BMP, and JPEG headers, plus caller-owned
BMP and single-frame GIF pixel decoders. GIF decoding validates palettes,
sub-blocks, transparency, LZW codes, output bounds, and frame geometry before
reporting success. It rejects unknown formats, truncated headers, and
dimensions above 8192 before pixel allocation or rasterization.

Evidence at commit `9e29d508`:

- Full `libwoven` host suite: 145 passed.
- Settings target warning-denying Clippy: passed.
- Formatting and diff checks: passed.

Animated/interlaced GIFs, streaming GIF input, hardware acceleration, and
media fuzz qualification remain outside this boundary. PNG/JPEG/WebP pixel
decoding and the separate chunk-delivery APIs are documented in the codec
implementation notes.
