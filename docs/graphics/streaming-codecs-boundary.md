# Streaming image codec boundary

`libwoven::codecs::ImageStream` and the internal seekable chunk reader let
PNG and JPEG decoders fetch encoded data through bounded `read_at` requests.
Requests are split at 1024 bytes, seek/peek/EOF behavior is checked, and the
decoded RGBA pixels remain caller-owned. PNG has an end-to-end chunk-reader
fixture; JPEG uses the same freestanding-compatible reader and adapter. BMP
uses the same boundary to fetch its header and each validated 24/32-bit row
without assembling the encoded file in a caller buffer.

WebP has incremental delivery through `StreamingDecoder`, but that backend
retains the encoded RIFF payload until final decode. GIF currently has a
bounded whole-slice single-frame decoder; animated GIF and streaming GIF are
not claimed.

Evidence:

- Full `libwoven` host suite: 174 passed.
- Host and `x86_64-unknown-none` warning-denying Clippy: passed.
- WovenImages freestanding target Clippy: passed.
