# Streaming image codec boundary

`libwoven::codecs::ImageStream` and the internal seekable chunk reader let
PNG and JPEG decoders fetch encoded data through bounded `read_at` requests.
Requests are split at 1024 bytes, seek/peek/EOF behavior is checked, and the
decoded RGBA pixels remain caller-owned. PNG has an end-to-end chunk-reader
fixture; JPEG uses the same freestanding-compatible reader and adapter. BMP
uses the same boundary to fetch its header and each validated 24/32-bit row
without assembling the encoded file in a caller buffer.

WebP has incremental delivery through `StreamingDecoder`, but that backend
retains the encoded RIFF payload until final decode. GIF now has a true
memory-bounded single-frame entry point: fixed palette/LZW tables and a
1024-byte source request are retained while pixels stream to the caller.
Interlaced row ordering is supported; animated GIF and multi-frame composition
are now integrated through `decode_gif_animation_stream` and a synchronous
`GifFrameSink`. `GifPlayback` supplies bounded caller-owned canvas/previous-
frame state and disposal/timing metadata without buffering a frame sequence.

Evidence:

- Full `libwoven` host suite: 180 passed, including exhaustive truncation
  prefix checks for BMP/GIF and malformed/truncated WebP stream cases.
- Host and `x86_64-unknown-none` warning-denying Clippy: passed.
- WovenImages freestanding target Clippy: passed.
