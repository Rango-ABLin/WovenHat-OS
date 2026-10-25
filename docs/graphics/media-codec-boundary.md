# Media and codec boundary

The current media increment adds `libwoven::media::parse_image_header`, a
bounded metadata parser for PNG, GIF, BMP, and JPEG headers. It rejects
unknown formats, truncated headers, and dimensions above 8192 before any
pixel allocation or rasterization.

Evidence at commit `9e29d508`:

- Full `libwoven` host suite: 145 passed.
- Settings target warning-denying Clippy: passed.
- Formatting and diff checks: passed.

This boundary does not claim complete pixel decoders, streaming codecs,
hardware acceleration, or media fuzz qualification.
