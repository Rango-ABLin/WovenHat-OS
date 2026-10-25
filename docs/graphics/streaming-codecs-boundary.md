# Streaming image codec boundary

`libwoven::codecs::ImageStream` and the internal seekable chunk reader let
PNG and JPEG decoders fetch encoded data through bounded `read_at` requests.
Requests are split at 1024 bytes, seek/peek/EOF behavior is checked, and the
decoded RGBA pixels remain caller-owned. PNG has an end-to-end chunk-reader
fixture; JPEG uses the same freestanding-compatible reader and adapter.

WebP continues to use its validated whole-slice backend because the selected
freestanding decoder does not expose an incremental reader API. This
increment therefore does not claim complete streaming WebP or animated-media
support.

Evidence:

- Full `libwoven` host suite: 166 passed.
- Host and `x86_64-unknown-none` warning-denying Clippy: passed.
- WovenImages freestanding target Clippy: passed.
