# WovenPDF native PDF reader — development increment

Status: initial inspection and narrow text-preview application only; **not a complete reader**.

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
