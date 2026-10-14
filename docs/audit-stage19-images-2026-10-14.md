# Stage 19 WovenImages audit — 2026-10-14

WovenImages is a separate static Ring-3 application. It lists a bounded
directory and recognizes PNG, JPEG, GIF, and BMP headers from at most 32 bytes
of a selected file. It never decodes unbounded pixel data or mutates storage;
unknown signatures fail closed as `unknown`.

The two host parser/header tests and warning-denying freestanding Clippy
passed. The signed external-application harness passed on 1, 2, and 4 CPUs
with exit 33; serial/QEMU evidence is preserved in
`audit-artifacts/woven-images-{1,2,4}cpu`.

The eight-application catalog and production-image integration consume the
package. Full image decoding, thumbnails, editing, and GPU-backed presentation
remain future graphics/application work.
