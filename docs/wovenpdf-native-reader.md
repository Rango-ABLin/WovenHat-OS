# WovenPDF native PDF reader — development increment

## 2026-10-25 FlateDecode increment

- Added `miniz_oxide` 0.9.1 with default features disabled to retain
  `no_std` support and avoid dynamic allocation for decompression.
- Wired `/FlateDecode` and `/Fl` to a caller-bounded output buffer using
  `decompress_slice_iter_to_slice`, with zlib wrapper and checksum verification.
- Added unit test source for a known zlib fixture, undersized output and
  checksum damage. These tests have **not been executed** in this environment.
- Limitations: no predictor decoding, filter pipelines, indirect stream
  lengths, object streams, or page content interpreter. The GUI is still
  a limited text preview and does not use the new decoder.
- Do not merge until the application builds with its freestanding target,
  cargo tests and Clippy pass, and QEMU 1/2/4 CPU acceptance is recorded.


## 2026-10-25 stream-to-filter integration

- The classic stream reader now exposes `stream_with_filter`, preserving
  the exact direct `/Length` extraction contract and identifying a single
  `/ASCIIHexDecode` or `/RunLengthDecode` filter (including PDF aliases).
- `decoded_stream` decodes these streams into a caller-provided bounded
  output slice. `raw_stream` still rejects any filter rather than
  silently presenting compressed data as uncompressed bytes.
- Unsupported filters and filter arrays reject explicitly. FlateDecode,
  PNG predictors, object streams, indirect `/Length`, page content
  execution, embedded fonts and faithful rendering remain incomplete.
- Unit tests were added but not run; no compiler was available here.


## 2026-10-25 bounded filter increment

- Added `src/filters.rs`: independent fixed-capacity ASCIIHexDecode and
  RunLengthDecode routines, plus Identity copies and unsupported-filter refusal.
- These filters are exported from the library but **not connected** to the
  stream dictionary pipeline or the graphical preview.
- Important: FlateDecode (used by most PDF documents), ASCII85Decode, LZW,
  predictors, font decoding and actual page graphics remain incomplete.
- Added unit test source for valid, malformed, truncated and output-capacity
  cases. No Rust toolchain was available to compile or execute tests.
- The app remains a non-production preview on draft PR #122.


## 2026-10-25 stream boundary increment

- Added `src/streams.rs` to resolve direct numeric `/Length` values in
  classic uncompressed PDF stream objects and return borrowed stream bytes.
- Stream extraction uses the declared length rather than scanning for
  `endstream` within binary content. Unsupported filters are rejected.
- Indirect stream lengths, FlateDecode, predictors, object-stream support,
  stream/object caching and actual page rendering remain unimplemented.
- The new module is not yet wired into the GUI preview. Tests are authored
  but not executed because Rust and QEMU tools are unavailable here.


## 2026-10-25 graphical preview increment

- `apps/woven-pdf/src/gui.rs` adds a WovenShell shared GraphicsSurface
  window and renders the existing **text-preview** output using Canvas.
- The entry point accepts `--gui SHELL_PID [PDF_PATH]`. Supported text is
  drawn in a bounded page-like panel; top/bottom bar clicks scroll text.
- This is not PDF page rendering. Font programs, text matrices, graphics paths,
  raster images, colorspaces, zoom, real pagination and faithful PDF layout are
  not implemented. The preview must not be advertised as full PDF support.
- This graphical path has NOT been compiled or run in QEMU in this session.
  A real desktop test plus 1/2/4 CPU matrix is required before merging.
- The shell is not automatically modified to launch WovenPDF, and file
  associations are intentionally not registered.


## 2026-10-25 continuation: object and page-tree foundations

- Added `objects.rs`: cross-reference-backed indirect-object header checks,
  dictionary /Name and /Reference getters, and explicit stream refusal.
- Added `pages.rs`: bounded recursive traversal of /Pages and /Kids arrays,
  with cycle, nesting-depth and output-capacity enforcement.
- These are scaffolding for the full parser. They are not yet connected to the
  WovenPDF CLI or to a page renderer. Classic-xref parsing, object streams,
  stream-length resolution, decompression, color, font decoding, pixel
  rendering and WovenShell GUI remain incomplete.
- Rust compiler, Clippy, and QEMU are unavailable in this execution environment,
  so the new source has **not been compile-tested or acceptance-tested**.
- Do not merge PR #122 or claim support for arbitrary PDF documents before
  the full release gates are run on a configured development machine.


Status: initial inspection, bounded lexical scanner, strict classic xref-table parsing, and narrow text-preview application only; **not a complete reader**.

## Source and rights

Mozilla [PDF.js](https://github.com/mozilla/pdf.js) is an Apache-2.0-licensed
JavaScript PDF engine. Its architecture and interoperability tests are useful
references. The Rust code in `apps/woven-pdf` is independently written; it does
not vendor or translate Mozilla's JavaScript. Do not present this as a complete
port of PDF.js. If any upstream code or fixtures are copied in future work,
retain license and NOTICE obligations and document the exact source revision.

To clone the reference on a development machine:

```bash
git clone https://github.com/mozilla/pdf.js.git third_party/pdfjs-reference
```

Keep the reference out of the freestanding WovenHat build graph.

## Current code

- `apps/woven-pdf` is a standalone Rust `no_std` Ring-3 ELF app.
- `src/lexer.rs` introduces a non-allocating bounded scanner for PDF tokens, with unit test sources covering malformed input, comments, names, strings, arrays, and dictionaries. Lexing does not yet resolve indirect objects.
- `src/xref.rs` reads strictly formatted classic `xref` tables via `startxref`, records bounded object offset/generation/free state entries, and rejects malformed offsets, duplicate object numbers, unsupported xref streams, and incremental `/Prev` trailers. It does not yet verify that object offsets point to correctly matching `obj` headers, parse the trailer `/Root`, or resolve references. The `--text` path remains the earlier proof-of-concept stream scan and does **not** consume the xref parser.
- `--info /mnt/example.pdf` validates bounded PDF header and EOF and prints version.
- `--text /mnt/example.pdf` previews literal text strings directly followed by
  `Tj` in streams without a declared filter.
- Input is bounded to 512 KiB; output is bounded to 16 KiB. File read occurs
  through `libwoven::open/read/close`. No external network requests.
- The parser is a subset-based educational starting point, not a validating
  general-purpose PDF parser. A real parser must resolve PDF objects, stream
  dictionaries, page trees, xref tables/streams, encodings and updates.

## Build and test on the development host

```bash
cd apps/woven-pdf
cargo +nightly test --target x86_64-unknown-linux-gnu --lib
cargo +nightly clippy --target x86_64-unknown-linux-gnu --lib -- -D warnings
cargo +nightly build --release
```

The target override for tests must be replaced with your actual installed host
triple if different. The freestanding release build uses the app's `.cargo/config.toml`
and must be exercised with the workspace's normal Ring-3 build/image tooling.

These commands are instructions; the authoring session has **not** executed
host Rust tests, kernel builds, or 1/2/4 CPU QEMU acceptance.

## Follow-up stages (not implemented)

1. Full spec-compliant lexer, xref tables, xref streams, indirect objects,
   incremental updates, object streams and trailer/root/page-tree traversal.
2. Bounded FlateDecode, then supported PDF filters with safe decompression
   and allocations under app-level memory/CPU resource quotas.
3. Proper content-operator execution, paths, paints, transformations, graphics
   state, clipping and text matrices; map into WovenRender.
4. PDF font mapping, embedded fonts, glyph selection, ToUnicode, font metrics,
   shaping and accessible text extraction.
5. Image XObjects, transparency, layers, patterns, gradients and color spaces.
6. WovenShell file-association launcher, canvas/window GUI, scroll, zoom,
   pagination, thumbnails, search and keyboard/accessibility.
7. Security: encrypted files, permission policy, WovenGuard isolation,
   malformed document fuzzing, streaming I/O and strict resource budgets.
8. PDF.js corpus and independent PDF conformance corpus comparisons;
   regression fixtures and QEMU real desktop tests on 1/2/4 CPUs.
9. Integration into image-pack manifests and desktop File Open flow only
   after the previous gates are green.

## Safety and limitations

Do not make WovenPDF default PDF handler until full parsing, renderer, sandbox,
security and acceptance gates pass. The uncompressed Tj preview is not
semantically correct for general fonts or PDF positioning and intentionally
does not claim to support compressed PDFs or full page rendering.

Agent coordination: this feature branch is isolated from the active Stage 07
graphics/image codec changes and does not modify kernel or libwoven.
