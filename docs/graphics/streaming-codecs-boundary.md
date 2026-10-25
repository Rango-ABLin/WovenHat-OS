# Streaming image codec boundary

`libwoven::codecs::ImageStream` and the internal seekable chunk reader let
PNG and JPEG decoders fetch encoded data through bounded `read_at` requests.
Requests are split at 1024 bytes, seek/peek/EOF behavior is checked, and the
decoded RGBA pixels remain caller-owned. PNG has an end-to-end chunk-reader
fixture; JPEG uses the same freestanding-compatible reader and adapter. BMP
uses the same boundary to fetch its header and each validated 24/32-bit row
without assembling the encoded file in a caller buffer.

WebP has incremental delivery through `StreamingDecoder`, but that backend
retains the encoded RIFF payload until final decode. For callers that require
bounded final decoding, `decode_webp_rgba_stream_bounded` accepts a
caller-owned encoded workspace, reads only into that workspace, and applies
dimension/file/memory limits before writing caller-owned RGBA output. The
pinned zenwebp backend documents that some internal allocations may ignore its
memory limit, so strict whole-process allocation certification remains an
explicit backend qualification item. GIF now has a true
memory-bounded single-frame entry point: fixed palette/LZW tables and a
1024-byte source request are retained while pixels stream to the caller.
Interlaced row ordering is supported; animated GIF and multi-frame composition
are now integrated through `decode_gif_animation_stream` and a synchronous
`GifFrameSink`. `GifPlayback` supplies bounded caller-owned canvas/previous-
frame state and disposal/timing metadata without buffering a frame sequence.

Evidence:

- Full `libwoven` host suite: 181 passed, including caller-workspace WebP and exhaustive truncation
  prefix checks for BMP/GIF and malformed/truncated WebP stream cases.
- Host and `x86_64-unknown-none` warning-denying Clippy: passed.
- WovenImages freestanding target Clippy: passed.
- WovenImages QEMU guest gate: passed on 1, 2, and 4 CPUs (exit 33), including
  streamed BMP, interlaced GIF, and two-frame GIF playback emission.
- External randomized codec campaign: `tools/codec-fuzz` completed 50,000
  iterations / 250,000 decoder cases. Strict whole-process WebP allocation
  certification passed for the pinned VP8L, VP8X/ALPH, and malformed fixture
  paths through `tools/webp-allocation-cert`: 702,669-byte peak live usage,
  no leak, and a hard 1 MiB process-wide budget. The compatibility streaming
  API still retains the encoded RIFF payload; universal proof of unexercised
  backend paths is not claimed.
