# WovenHat OS Architecture & Codebase Guide

## Graphics transformed-rectangle corner update (2026-10-19)

`Renderer::transformed_rect` derives its exclusive far edges with checked
conversion and saturation before calling the fixed-point transform. This
preserves the clipped triangle path while preventing dimension narrowing from
wrapping geometry.

## Graphics image blit update (2026-10-19)

`Renderer::image` and `image_blended` clip their iteration to the Canvas and
derive source offsets from the clipped origin. Valid source buffers retain
their existing success semantics while off-screen image area is not rasterized.

## Graphics stroke geometry update (2026-10-19)

`Renderer::stroke` computes far edges with checked conversion and saturation,
then delegates each edge to the clipped fill path. This preserves normal
outline behavior while keeping extreme rectangle dimensions bounded.

## Graphics gradient clipping update (2026-10-19)

`Renderer::vertical_gradient` and `Renderer::horizontal_gradient` iterate
only over the Canvas intersection, but derive interpolation from the full
logical rectangle so off-screen origins do not change visual output. The
shared Canvas clip helper uses i128 bounds to reject extreme dimensions
without overflow.

## Graphics transformed-triangle clipping update (2026-10-19)

The Renderer transformed-rectangle triangle helper clips its iteration bounds
to the Canvas before rasterization, sharing the widened edge arithmetic and
safe pixel boundary used by software 3D.

## Graphics software-3D clipping update (2026-10-19)

`Renderer::triangle_3d` bounds raster iteration by both the Canvas and depth
buffer rectangles and uses i128 edge/interpolation intermediates. This keeps
caller-owned software 3D safe for extreme projected coordinates while leaving
the future hardware/GPU backend boundary unchanged.

## Graphics contextual GPOS format-2 target update (2026-10-19)

The chaining format-2 path keeps first-input class coverage separate from
nested target coverage, allowing bounded single-positioning records to target
later input sequence indices without weakening class or table validation.

## Graphics contextual GPOS format-1 pair extension update (2026-10-19)

The chaining format-1 nested lookup-2 pair path is regression-tested through a
checked type-9 extension wrapper, reusing the same target, partner, pair-set
and value-record bounds.

## Graphics contextual GPOS format-1 pair update (2026-10-19)

The chaining format-1 path now shares the checked nested lookup-2 format-1
pair-positioning implementation with chaining format 2. Target coverage,
partner glyph, pair-set and value-record bounds are validated before applying
the first-glyph adjustment.

## Graphics contextual GPOS format-1 target update (2026-10-19)

The bounded chaining format-1 path keeps first-input coverage matching separate
from nested target coverage, so records targeting sequence indices 0 through 4
apply their single-positioning value to the selected glyph.

## Graphics contextual GPOS class-matrix extension update (2026-10-19)

Nested lookup-2 format-2 class-matrix pair positioning is regression-tested
through a checked type-9 extension wrapper. The wrapper target and nested
subtable bounds feed the same validated class/matrix/value-record path.

## Graphics contextual GPOS class-matrix pair update (2026-10-19)

Nested lookup-2 format-2 class-matrix pair positioning now shares the bounded
class-rule path. The selected target and partner classes, matrix dimensions,
value-record size and matrix extents are validated before applying the first
value record; malformed or unsupported nested forms fail closed.

## Graphics contextual GPOS class-rule pair update (2026-10-19)

Nested lookup-2 format-1 pair positioning is now available from the bounded
GPOS chaining format-2 class-rule path. Pair-set, partner glyph, value-format
and record extents are checked before applying the first-glyph adjustment;
broader nested pair/class forms remain outside this slice.

## Graphics contextual GPOS chaining format-2 update (2026-10-19)

The GPOS lookup type-8 format-2 path validates class definitions, class-set
rules, bounded backtrack/input/lookahead classes and nested lookup records.
Matched rules share the checked nested lookup-1 single-positioning path,
including type-9 wrappers; nested pair/class positioning remains outside this
slice.

## Graphics contextual GPOS chaining format-1 update (2026-10-19)

The GPOS lookup type-8 format-1 path validates bounded glyph-rule chains,
backtrack/lookahead context and nested lookup-1 single positioning. Direct and
type-9-wrapped nested lookups share the checked value-record path; nested pair
and class positioning remain outside this slice.

## Graphics contextual GPOS chaining update (2026-10-19)

GPOS lookup type 8 chaining-context format 3 shares the bounded contextual
dispatcher with type 7, including checked input/context coverage, nested
single/pair positioning, class matrices and extension wrappers.

## Graphics contextual GPOS class-pair update (2026-10-19)

Nested contextual type-2 pair positioning supports format-2 class definitions
and class-matrix value records with checked coverage, class and table bounds.
The bounded target/partner selection is shared with the contextual input
sequence path; broader nested lookup application remains outside this slice.

## Graphics contextual GPOS extension update (2026-10-19)

Contextual GPOS dispatch accepts a checked type-9 extension wrapper around a
nested type-7 subtable and then applies the existing bounded contextual paths.
Extension offsets and nested table bounds are validated before dispatch.

## Graphics two-coverage contextual GPOS guard update (2026-10-19)

The bounded contextual format-3 matcher validates up to two backtrack and two
lookahead coverage records using independently supplied neighboring glyphs.
The implementation remains explicitly bounded and fails closed when required
neighbors are unavailable; longer context is not implied.

## Graphics multi-input contextual GPOS pair update (2026-10-19)

Nested type-2 pair positioning can target each supported contextual input
index, using the following input glyph when available or the next shaped glyph
outside the matched sequence. The target adjustment and later-origin advance
propagation remain bounded and checked; broader nested lookup application is
not implied.

## Graphics contextual GPOS sequence-index-1 pair update (2026-10-19)

The bounded two-input contextual dispatcher can apply a nested type-2 pair
lookup to sequence index one, using the following shaped glyph as its checked
pair partner and propagating the resulting adjustment from that target.
Nested pair targets beyond index one remain outside this path.

## Graphics five-input contextual GPOS update (2026-10-19)

The bounded format-3 dispatcher supports five input coverages and can apply
nested type-1 positioning to sequence index four, propagating advance deltas
through later shaped glyph origins. Pair positioning remains restricted to the
two-input, sequence-index-zero path; longer context and broader nested lookup
application remain outside this implementation.

## Graphics four-input contextual GPOS update (2026-10-19)

The bounded format-3 dispatcher supports four input coverages and can apply
nested type-1 positioning to sequence index three, propagating advance deltas
through later shaped glyph origins. Pair positioning remains restricted to the
two-input, sequence-index-zero path; longer context and broader nested lookup
application remain outside this implementation.

## Graphics three-input contextual GPOS update (2026-10-19)

The bounded format-3 dispatcher supports three input coverages and can apply
nested type-1 positioning to sequence index two, propagating advance deltas
through later shaped glyph origins. Pair positioning remains restricted to the
two-input, sequence-index-zero path; longer context and broader nested lookup
application remain outside this implementation.

## Graphics single-input contextual GPOS update (2026-10-19)

The contextual format-3 dispatcher supports one-input coverage records through
the shaped-glyph pass, applying nested type-1 positioning and propagating
advance deltas. Adjacent context remains bounded to one checked backtrack and
one lookahead glyph; broader contextual lookup semantics remain outside this
path.

## Graphics contextual GPOS sequence-index-1 update (2026-10-19)

The bounded contextual format-3 dispatcher can apply nested type-1 single
positioning to sequence index one and propagate its advance through following
shaped glyph origins. Nested pair positioning remains restricted to sequence
index zero; later indices and broader contextual lookup application remain
outside this path.

## Graphics contextual GPOS composition update (2026-10-19)

Contextual format-3 dispatch accumulates multiple nested single or pair value
records targeting sequence index zero with saturating placement/advance
arithmetic. Other sequence indices and broader contextual lookup application
remain outside this bounded path.

## Graphics contextual GPOS guard update (2026-10-19)

Format-3 contextual positioning can validate one previous and one following
coverage against adjacent shaped glyphs, while retaining the context-free
two-input path. The bounded dispatch remains limited to sequence index zero;
longer context and multi-record lookup application are not implied.

## Graphics nested contextual GPOS update (2026-10-19)

Contextual format-3 records may now dispatch a bounded nested type-2
format-1 pair lookup for the first input sequence position, alongside nested
single adjustment. Pair-set coverage and value-record bounds are checked;
general contextual backtrack/lookahead and multi-record dispatch remain
outside this slice.

## Graphics contextual GPOS update (2026-10-19)

The bounded contextual positioning path recognizes format-3 records with two
input coverages and no backtrack/lookahead, then applies a nested type-1
format-1 adjustment to the first input. Nested extension wrappers are
validated through the shared lookup resolver; broader nested pair/context
composition is intentionally not implied.

## Graphics GPOS mark-to-ligature update (2026-10-19)

Lookup type 5 format 1 decodes mark arrays and bounded ligature component
records, resolving the selected class anchor against the mark anchor. The
current shaped-glyph representation does not encode a following mark's target
component, so the implementation selects the final ligature component; a
future cluster/component-carrying ABI can refine that choice.

## Graphics GPOS attachment extension update (2026-10-19)

Attachment lookup paths accept type-9 extension wrappers only when the
extension format and nested lookup type match the requested mark-to-base,
mark-to-mark or cursive path. The nested subtable offset and all enclosing
bounds are checked before decoding.

## Graphics GPOS script/LangSys attachment update (2026-10-19)

Mark-to-base, mark-to-mark and cursive positioning share a bounded feature
selector that resolves the requested script and LangSys records, including
required features. An absent script or language produces no attachment;
default shaping enumerates the feature list as before.

## Graphics GPOS lookup composition update (2026-10-19)

Selected `kern` lookups are evaluated in declared order. Matching single and
pair value records accumulate with saturating placement/advance arithmetic;
single and pair queries remain separated by the internal sentinel, and every
lookup/subtable path retains bounded validation.

## Graphics GPOS outline-resolved anchor update (2026-10-19)

Format-2 anchors resolve their contour-point index through `outline_points_glyph`
before attachment deltas are applied. Point count, glyph outline and table
bounds are checked, and missing or malformed outlines fail closed.

## Graphics GPOS attachment anchor update (2026-10-19)

Format-3 anchor decoding, including optional ppem device deltas, is shared by
mark-to-base, mark-to-mark and cursive attachment. Format-2 contour-point
anchors use the bounded outline decoder.

## Graphics GPOS format-3 anchor update (2026-10-19)

Mark-to-base anchor decoding supports format 3 with optional ppem device
deltas. Format 1 remains coordinate-only; format 2 contour-point anchors use
the bounded glyph outline points.

## Graphics GPOS mark-to-mark update (2026-10-19)

GPOS type-6 format-1 `mkmk` records now connect a second mark to a first mark
using validated class-matched anchors. The shaping pass applies the first
anchor minus second anchor delta to the second mark origin while retaining its
advance; alternate anchor formats remain outside this slice.

## Graphics GPOS cursive update (2026-10-19)

GPOS type-3 format-1 cursive records now connect adjacent covered glyphs using
validated entry/exit anchors. The right glyph receives the exit-minus-entry
origin delta and retains its advance; other attachment and anchor formats are
outside this slice.

## Graphics GPOS mark-to-base update (2026-10-19)

The shaping pass recognizes GPOS type-4 format-1 mark-to-base records in
`mark` and `mkmk` features. It validates coverage, class and anchor arrays,
then applies base-minus-mark anchor deltas to the mark origin while retaining
its advance. Other attachment and anchor formats remain outside this slice.

## Graphics GPOS single-adjustment update (2026-10-19)

GPOS type-1 format-1/2 subtables now produce bounded placement and advance
adjustments through the existing value-record/device-table decoder. The
shaping pass applies the result per glyph and shifts following origins by
advance deltas; attachment and other positioning lookup types remain outside
this slice.

## Graphics GPOS extension update (2026-10-19)

The GPOS shaping path unwraps type 9 extension records for nested type-2 pair
positioning subtables. The existing format-1/format-2 value-record parser and
device-table ppem handling are reused after validating the extension offset;
other positioning lookup types remain fail-closed.

## Graphics GSUB extension class update (2026-10-19)

Type 7 extension records for lookup types 5 and 6 can now route format-2
class-based contextual/chaining subtables through the existing checked class
matchers. The extension path preserves bounded class definitions, rules and
nested lookup references; malformed data remains fail-closed.

## Graphics GSUB extension contextual update (2026-10-19)

Type 7 extension records for lookup types 5 and 6 can unwrap bounded format-3
contextual/chaining subtables and route them through the existing coverage
matcher. Nested lookup references remain bounded by the active lookup list;
class-based extension forms remain fail-closed until separately implemented.

## Graphics GSUB extension ligature update (2026-10-19)

Type 7 extension records can now unwrap bounded type-4 ligature subtables in
addition to type-1 single substitutions. Matching returns the consumed
component count to the shaping pipeline; malformed or unsupported extension
forms remain fail-closed.

## Graphics GSUB extension update (2026-10-19)

The bounded GSUB shaping path unwraps type 7 extension records for nested
single substitutions, validating the extension offset and the nested format 1
or 2 coverage/substitute data before applying it. Other extension lookup types
remain fail-closed until separately implemented.

## Graphics GSUB composition update (2026-10-19)

The shaping path runs selected GSUB results through an eight-pass bounded
local pipeline. A single substitution can therefore feed the next selected
lookup, while ligature consumption ends the local sequence and repeated
no-op/cycle behavior cannot run indefinitely. Complete OpenType lookup
ordering, broader nested composition and hardware GPU acceleration remain
outside this slice.

## Graphics class-based chaining GSUB update (2026-10-19)

GSUB lookup type 6 format 2 now validates separate backtrack, input and
lookahead class definitions and applies matching chain rules with a nested
type-1 substitution at sequence index zero. Class-set, rule, sequence, lookup
and glyph ranges are bounded. Broader nested lookup composition, complete
feature ordering and hardware GPU acceleration remain outside this slice.

## Graphics chaining GSUB update (2026-10-19)

GSUB lookup type 6 format 3 now shares the bounded backtrack/input/lookahead
coverage matcher with contextual shaping. A matching chaining record can
invoke a nested type-1 substitution at sequence index zero; all table and
glyph ranges are validated. Class-based chaining, broader nested lookup
composition, complete feature ordering and hardware GPU acceleration remain
outside this slice.

## Graphics class-based contextual GSUB update (2026-10-19)

GSUB lookup type 5 format 2 now supports checked class-definition lookup,
class-set rule selection, subsequent input-class matching and a nested type-1
substitution at sequence index zero. Rule, class-set, nested-lookup and glyph
ranges are bounded. Chaining substitution, complete feature ordering and
hardware GPU acceleration remain outside this slice.

## Graphics contextual GSUB update (2026-10-19)

The bounded GSUB path supports contextual lookup type 5 format 3 coverage
matching across backtrack, input and lookahead glyphs. A matching record may
invoke a nested type-1 single substitution at sequence index zero; all
coverage, sequence, lookup and glyph ranges are validated. Class-based/contextual
formats 1/2, chaining substitution, complete feature ordering and hardware GPU
acceleration remain outside this slice.

## Graphics GSUB single-substitution update (2026-10-19)

The bounded GSUB path accepts lookup type 1 formats 1 and 2 for selected
Script/LangSys features (`ccmp`, `locl`, `smcp`, `subs`, `sups`, `case`, and
`cpsp`). Coverage indexes, format-2 substitute arrays and resulting glyph IDs
are range-checked before a shaped glyph is emitted. Contextual/chaining GSUB,
complete feature ordering and hardware GPU acceleration remain outside this
slice.

## Graphics named LangSys update (2026-10-19)

`TextShapingOptions` carries optional ScriptList and named LangSys tags plus
the ppem needed by device tables. The shaping path resolves the requested
language record with checked bounds for both GSUB `liga`/`rlig` and GPOS
`kern`; a missing language record does not silently use the default or another
language. Contextual/complex GSUB/GPOS, broader script data and hardware GPU
acceleration remain outside this slice.

## Graphics script-scoped GPOS update (2026-10-19)

The script-aware shaping path now passes the selected ScriptList/default
LangSys through GPOS `kern` pair lookup selection as well as GSUB. The
`shape_text_ligatures_with_script_and_ppem` entry point makes both the script
tag and ppem required when device deltas are needed. Missing scripts do not
fall back to another script. Named language systems, contextual/complex
GSUB/GPOS and hardware GPU acceleration remain outside this slice.

## Graphics GSUB script-selection update (2026-10-19)

`TrueTypeFace::shape_text_ligatures_with_script` selects a requested ScriptList
record and its default LangSys before resolving bounded `liga`/`rlig` feature
records and type-1 lookups. It does not fall back to another script when the
requested tag is absent. The legacy shaping API retains its existing feature
list behavior. Language-specific LangSys selection, GPOS script scoping,
contextual/complex GSUB and hardware GPU acceleration remain outside this
slice.

## Graphics GPOS device-table update (2026-10-18)

`TrueTypeFace::shape_text_ligatures_with_ppem` is the explicit ppem-aware
entry point for GPOS device records. Formats 1, 2 and 3 are decoded with
checked ranges and signed packed deltas; each delta augments its matching
x/y-placement or x/y-advance value before the existing font-unit scaling and
origin propagation. The no-ppem shaping path returns a fail-closed error when
a device record is encountered. Variation-index device semantics, full script
shaping and hardware GPU acceleration remain outside this slice.

## Graphics GPOS y-value update (2026-10-18)

GPOS pair records now carry bounded y-placement and y-advance values through
the shaped glyph origin path, with font-unit-to-pixel scaling and propagation
to following origins. Device-table deltas and full vertical layout semantics
are not implied; the QEMU/VirGL hardware presentation boundary is unchanged.

## Graphics GPOS class positioning update (2026-10-18)

GPOS pair positioning now supports bounded format-2 class matrices in addition
to format 1 pair sets. Class-definition formats 1/2 and matrix dimensions are
validated before applying scaled x adjustments; unsupported value/device
semantics remain fail-closed. Full script shaping and the QEMU/VirGL hardware
presentation boundary are unchanged.

## Graphics GPOS positioning update (2026-10-18)

The shaped text path optionally follows the GPOS `kern` feature to type-2
pair-positioning format 1 and applies bounded x-placement/x-advance records in
scaled pixels. Unsupported lookup/value formats remain fail-closed; full GPOS
device tables and script shaping are not implied. The QEMU/VirGL hardware
presentation boundary is unchanged.

## Graphics variation-aware shaping update (2026-10-18)

Shaped glyph IDs can now carry normalized gvar coordinates through metric
calculation, outline construction and antialiased rasterization. The base path
does not apply variation deltas; the variation path applies them once and
validates coordinate/table bounds. Full script shaping and the QEMU/VirGL
hardware presentation boundary remain separate.

## Graphics shaped-glyph raster update (2026-10-18)

The font boundary now supports direct glyph-ID outline decoding and
antialiased rasterization, with `render_shaped_text` carrying GSUB-selected
IDs from shaping into the renderer. Unicode-character wrappers remain intact;
variation-aware shaped rasterization and full script shaping are not implied.
The QEMU/VirGL hardware presentation boundary is unchanged.

## Graphics GSUB shaping update (2026-10-18)

The borrowed TrueType/OpenType boundary now exposes `ShapedGlyph` and a
bounded GSUB ligature path. It follows `liga`/`rlig` feature records to type-1
ligature lookups, validates coverage and component sequences, and derives the
selected glyph's horizontal metric. It does not yet rasterize arbitrary shaped
glyph IDs or implement full script shaping; the QEMU/VirGL hardware boundary
is unchanged.

## Graphics directional text update (2026-10-18)

`TrueTypeFace::layout_text_bidi` resolves bounded strong RTL/LTR runs and
neutral characters before emitting visual placements. `TextDirection::classify`
recognizes common RTL blocks and Unicode alphanumerics; punctuation and
combining marks remain neutral. Full UAX#9, script shaping, and OpenType
feature application are not implied, and the QEMU/VirGL hardware boundary is
unchanged.

## Graphics hint control-flow update (2026-10-18)

The bounded TrueType interpreter now recognizes `IF`/`ELSE`/`EIF` and
relative `JMPR`/`JROT`/`JROF`. Branch scanning is instruction-span-aware so
immediate PUSH data cannot be mistaken for opcodes; nesting and jump targets
are bounded by the copied program, and malformed control flow fails closed.
Function calls, full projection math and hint interpolation are not implied.

## Graphics storage/CVT hint update (2026-10-18)

The bounded hint interpreter exposes caller-owned CVT and storage resources:
`MIAP`/`MDAP` validate point and CVT indices, while `WS`/`RS` validate storage
indices. Missing resources and unsupported instructions fail closed; the
software renderer and QEMU/VirGL presentation boundary are unchanged.

## Graphics CVT hint update (2026-10-18)

The bounded hint interpreter now supports `MDAP` point selection and `MIAP`
placement from caller-owned CVT values along the active freedom axis. Missing
CVT data and unsupported hint resources fail closed; software rendering and
the QEMU/VirGL presentation boundary are unchanged.

## Graphics hint-state execution update (2026-10-18)

The font boundary now executes a bounded coordinate-independent hint subset
covering vector selection, rounding state, stack primitives and PUSHB/PUSHW.
Point movement, storage, CVT and control flow fail closed; the software
renderer and QEMU/VirGL presentation boundary are unchanged.

## Graphics hinting boundary (2026-10-18)

`TrueTypeFace::glyph_hint_program` provides a bounded, validated copy of
simple-glyph instruction bytes; execution is deliberately absent, and
composite programs fail closed. The software raster path and QEMU/VirGL
presentation boundary remain unchanged.

## Graphics phantom metrics update (2026-10-18)

`TrueTypeFace::advance_width_with_variation` retains simple-glyph phantom
points while applying `gvar` deltas, deriving varied advance width from the
right and left phantom positions. Incomplete legacy LSB storage preserves the
base advance path and fails closed for variation metrics; QEMU/VirGL output is
unchanged.

## Graphics composite variation update (2026-10-18)

Composite `gvar` processing now treats packed variation point numbers as
component indices, recursively applies nested component variation, and shifts
each bounded component output range by its checked X/Y delta. Phantom metrics
and hinting remain outside this renderer boundary; QEMU/VirGL presentation is
unchanged.

## Graphics shared-point variation update (2026-10-18)

Simple-glyph `gvar` tuples may now use a checked glyph-level packed shared
point set, including its all-points form. Composite variation remains
unsupported and fails closed; the QEMU/VirGL hardware presentation boundary
is unchanged.

## Graphics variation update (2026-10-18)

The simple-glyph `gvar` path now supports embedded-peak intermediate regions
using checked start/peak/end ranges and piecewise normalized F2DOT14 scalar
blending. Shared-point sets and composite variation still fail closed; the
QEMU/VirGL hardware presentation boundary is unchanged.

## Graphics font variation boundary (2026-10-18)

`libwoven::graphics::TrueTypeFace::outline_points_with_variation` applies
embedded-peak `gvar` tuples to simple glyph points using normalized F2DOT14
coordinates. Composite glyph variation, intermediate regions and shared-point
sets fail closed until separately implemented; the compositor and QEMU/VirGL
hardware boundary are unchanged.

## Graphics backend boundary (2026-10-18)

The kernel VirtIO-GPU path keeps the existing software/2D scanout as the
accepted fallback. Its Stage 20.14 VIRGL probe negotiates optional resource
blobs, creates bounded context/resource command streams, and can encode a
surface/framebuffer/RGBA-clear sequence. QEMU's default GL device accepts the
transport path but does not accept the tested 3D resource through classic
`SET_SCANOUT`; its opt-in guest blob path is not accepted by the VirGL context.
Blob-backed presentation therefore remains capability-gated and unclaimed
until a visual QEMU frame is verified.

## Live boot log startup candidate (2026-10-16; physical retry correction 2026-10-17)

Production boot queues Stage 13.19 USB live-log setup immediately before
attempting to start the Stage 20.15 WovenShell session. File creation and the
initial FAT flush run in a background I/O service, because they may take time
on physical USB media. The worker is queued even before USB enumeration is
visible and retries discovery/setup failures. Physical USB enumeration now
also retries every 300 timer ticks until a storage device is actually
published; this covers devices that settle after the first probe and avoids
the existing-HID fast path suppressing later mass-storage discovery. The
acceptance self-test retains the synchronous start path. Focused Clippy and
Stage 13.18/13.19 QEMU gates pass, but physical USB boot still returned no
`LOGS/BOOT0001.TXT` on two sticks; serial capture remains required. See the
[startup audit](audit-live-boot-log-startup-2026-10-16.md).

Driver bring-up and xHCI initialization now emit phase checkpoints before
potentially long waits. Serial transmit polling is bounded in all builds, with
log bytes retained in the in-memory ring even if no UART responds. This does
not make a USB log possible while xHCI is hung; external serial capture is
needed for a permanently stalled controller.

## Stages 1–10 release evidence integrity (2026-10-16)

Memory, storage, live-network and Stage 6 hotplug QEMU gates share
`scripts/qemu_validation.py`: exact successful exit, line-anchored required
markers, and rejection of tagged failure/timeout or kernel panic anywhere in
serial output. Expected user faults remain valid isolation evidence. Negative
host tests exercise every consumer's imported validator. This changes no kernel
ABI or lifecycle; full preservation is pending, not release acceptance. See
the foundation release-validation audit and stages 1–10 release ledger.

## Stage 19 WovenCalculator (2026-10-14)

`apps/woven-calculator` is a separate Ring-3 crate with a bounded checked
arithmetic core, a console mode, and an optional GUI mode. It uses the same
static ET_EXEC linker contract and WAPP signing path as the existing external
application. Its 1/2/4-CPU signed QEMU evidence is recorded in the Stage 19
calculator audit. This is the first system application increment; the rest of
the roadmap application suite and catalog/boot-image installation remain open.

`apps/woven-terminal` is the second application increment. It provides bounded
console commands over `libwoven` file descriptors, delegates arithmetic to the
calculator core, and passes signed 1/2/4-CPU Ring-3 execution. Its listing and
file-output caps are documented in the Stage 19 terminal audit; interactive
stdin and boot-catalog installation remain open.

`apps/woven-files` is the third application increment: a read-only bounded
directory lister and file previewer using the same verified external-image
path. It passes signed 1/2/4-CPU execution; GUI, boot catalog, and mutation
workflows remain open. See the Stage 19 WovenFiles audit.

`apps/woven-monitor` is the fourth increment. It reports bounded UTC clock,
authorized compositor geometry, and `/mnt` entry-count diagnostics without
write or privilege authority. Its signed 1/2/4-CPU execution evidence is
recorded in the Stage 19 WovenMonitor audit.

`apps/woven-settings` is the fifth increment. It reports bounded process
identity, UTC clock, and compositor display policy without mutating settings or
requesting privileged authority. Its signed 1/2/4-CPU execution evidence is
recorded in the Stage 19 WovenSettings audit.

`apps/woven-security` is the sixth increment. It reports bounded process
identity and security-ledger entries when the existing inspect authority is
available, without granting or revoking policy. Its signed 1/2/4-CPU execution
evidence is recorded in the Stage 19 WovenSecurity audit.

`apps/woven-editor` is the seventh increment. It provides bounded text preview
and capability-enforced file writing through the safe `libwoven` file-write
wrapper. Its signed 1/2/4-CPU execution evidence is recorded in the Stage 19
WovenEditor audit.

`apps/woven-images` is the eighth increment. It provides bounded image
inventory and common-header inspection without decoding untrusted pixel data.
Its signed 1/2/4-CPU execution evidence is recorded in the Stage 19 WovenImages
audit.

`apps/woven-update` is the ninth increment. It provides bounded read-only
update preflight and leaves installation, activation, and rollback to a future
privileged update service. Its signed 1/2/4-CPU execution evidence is recorded
in the Stage 19 WovenUpdate audit.

`apps/woven-store` is the tenth increment. It provides bounded read-only
installed-catalog browsing and leaves package acquisition and mutation to a
future privileged service. Its signed 1/2/4-CPU execution evidence is recorded
in the Stage 19 WovenStore audit.

`scripts/package-system-apps.py` is the release packaging boundary for these
ten accepted apps. It builds each app in its own workspace, signs the ELF
with WAPP v1, checks the envelope before publication, and emits hashed package
and `/share/applications` catalog artifacts in one install-plan manifest. The
manifest is consumed by the feature-gated multi-package RAM installer; the
remaining roadmap applications and full GUI integration remain open.

The feature-gated `stage19-app-catalog-test` now exercises the install-plan
boundary for all ten accepted packages together. It verifies each WAPP,
rejects a mutation, installs and reads each package back through the VFS,
passes the bytes through the verified ELF loader, and publishes the matching
catalog descriptor only after those checks. The 1/2/4-CPU evidence is in the
Stage 19 system-app catalog audit. This is not ordinary-boot installation:
rollback, trusted catalog publication, and image/installer ownership remain
required for production deployment.

The `system-apps` image feature is the first production-image consumer. It
requires a compile-time provisioned application public key, validates every
WAPP and normal ELF load before VFS mutation, checks path collisions, and
rolls back entries created by a failed publication. Acceptance harnesses pass
the deterministic test key explicitly; production provisioning must replace
it. The current publication is RAM-backed at boot, so persistent update,
power-loss rollback, and key lifecycle policy remain open.

## Release validation (2026-10-14)

The repository release chain passed on the current tree, including warning-
denying kernel/host Clippy, all host integration tests, 1/2/4-CPU debug
memory/storage/network gates, legacy PIC, 4-CPU release gates, release build,
and shell smoke. It now also runs app-specific warning-denying Clippy and the
signed five-application catalog gate on 1/2/4 CPUs. The release ZIP includes
the signed WAPP packages, catalog descriptors, and complete SHA-256 inventory.
The durable report and image manifest are recorded by the
[release validation audit](audit-release-validation-2026-10-14.md) and the
[system-app release audit](audit-release-system-apps-2026-10-14.md). This is
software/QEMU qualification only; native hardware and destructive media limits
remain explicit.

## Stage 14.5D network revalidation (2026-10-14)

The live WovenGuard transport/path suite passes on 1/2/4 CPUs, including
DHCP, DNS, ICMP, host-verified UDP/TCP round trips, and SMP/TLB checks. The
evidence is recorded in the [Stage 14.5D revalidation audit](audit-stage14-5d-revalidation-2026-10-14.md).
It qualifies the QEMU virtio boundary only; native NIC/radio and physical-link
behavior remain hardware work.

## Stage 13.16 report-protocol HID: tablets, pens, touch, game controllers (2026-10-14)

`kernel/src/hid.rs` is a pure module (no allocation, no kernel state) that
host tests compile directly (`tests/hid_reports.rs`).

**`parse`.** It reads a HID report descriptor into a bounded `Layout`:
- the top-level application: mouse, pointer, pen, touch screen, touchpad,
  joystick or game pad;
- up to 48 input fields, each with its usage, report ID, bit offset and
  size, logical range, and whether it is relative.

It handles report IDs, usage ranges, push and pop, padding, and long
items. A malformed descriptor, or one with an application it doesn't
decode, is refused.

**`Decoder`.** It turns reports into `Input` events and keeps the state it
needs to report only changes:
- relative and absolute pointer motion;
- wheel;
- button edges;
- pen position, pressure and contact;
- per-contact touch, with each contact's lift;
- game-controller buttons, axes and hat.

Positions are scaled to 0..=32767 and pressure to 0..=1023.

**xHCI.**
- `parse_hid_interface` reads the report descriptor's length from the HID
  class descriptor.
- A HID interface that isn't a boot keyboard or mouse has its report
  descriptor fetched (GET_DESCRIPTOR, report type, addressed to the
  interface) and parsed. It is kept as an input device only if the layout
  decodes.
- Reports up to 64 bytes are copied.
- The `usb-input` worker builds one `Decoder` per report device when it
  starts, and maps decoded inputs to WovenInput events, still outside the
  controller lock.

**WovenInput and userspace.** `Scroll`, `PointerAbsolute`, `Touch`, `Pen`
and `GameController` are production events, encoded to userspace as input
kinds 9–13 and decoded by libwoven's `InputEvent`.

## Stage 13.15 WovenAudio playback for userspace (2026-10-14)

When bring-up starts `hda0`, it calls `hda::start_output("hda0")`.
1. **Output setup.** `prepare_output` finds the output route (converter,
   pin, amplifier), enables the pin, sets the amplifier to 75%, and
   allocates the playback stream's memory once: a buffer descriptor list
   and 16 DMA pages, 64 KiB in all.
2. **Interrupt.** The controller's MSI-X or MSI vector is bound through the
   driver manager.

**Syscalls.**
- **203 `AudioInfo`:** the output format, 48 kHz 16-bit stereo, and the
  largest buffer one play accepts.
- **204 `AudioPlay`:** needs the new `AudioPlayback` capability.
  1. It copies the PCM into kernel memory, because user pages may fault and
     the controller's IRQ lock must not be held across a fault.
  2. Only one playback runs at a time. The calling task registers as the
     vector's waiter.
  3. `begin_playback` fills the pages and the descriptor list, with
     interrupt-on-completion on the last entry only, and starts the stream
     with IOCE set. The caller then sleeps in `wait_for_event_until`.
  4. The stream's completion interrupt wakes it. It checks BCIS, stops the
     stream and clears its waiter registration.
  5. Every attempt is recorded in the security ledger (`Action::AudioPlay`).
- **205 `AudioSetVolume`:** needs `AudioPlayback`; sets the output
  amplifier gain, and 0 mutes.

libwoven exposes `audio_info`, `audio_play` and `audio_set_volume`.

**Capability.** `AudioPlayback` is capability 24 and permission 16
(`Permission::Audio`). It is in the system-service ceiling and in the user
ceiling, so the permission broker can grant it. Recording is not
implemented yet; it will use the existing broker-gated `Microphone`
capability.

## Stage 13.14 storage on NVMe and AHCI (2026-10-14)

The storage layer reaches its disk only through `block_io`:
- the block-I/O worker's `direct_primary`;
- the `PrimaryAta` handle, which storage uses for persistence, journals and
  swap;
- the FAT32 mount in `storage::mount_ata_root`.

`block_io` now has a selectable root device, `RootDevice::{Ata, Nvme,
Ahci}`, and every one of those paths goes through `block_io::with_root`.
The default stays ATA, so every existing configuration behaves as before.

A normal boot with no ATA volume mounted does this after driver bring-up:
1. `drivers::storage_root` picks the first ready `nvme0` or `ahci0`.
2. `block_io::select_root` makes it the root device.
3. The volume is mounted at `/mnt` through the same storage code, so
   persistence, the data journal and swap all run on the NVMe or AHCI disk.

`storage.rs` no longer refers to the ATA driver directly.

**Still ATA-only.** WovenFS (`wfs.rs`) remains on the ATA primary master.

## Stage 13.13 USB keyboards and mice (2026-10-14)

When bring-up starts `xhci0`, it calls `xhci::start_input("xhci0")`.

**Enumeration.** Every connected root port gets its own reset and slot.
`enumerate_hid` runs on the controller's active fields, which are then
moved into a per-device `InputDevice`. Each device keeps its own slot, EP0
ring, contexts, interrupt ring and report buffer, for up to four keyboards
and mice. Each one is switched to report-on-change (`SET_IDLE 0`) and has
one interrupt-IN transfer armed.

**Interrupts.**
- An MSI-X or MSI vector is bound through the driver manager.
- `interrupts::set_pci_vector_waiter` registers the `usb-input` worker as
  that vector's waiter. The generic PCI handler, after publishing the work
  flag, wakes the waiter through the scheduler event latch, so a wakeup
  cannot be lost.
- Interrupter 0 is enabled, moderated to one interrupt per millisecond.

**The worker.**
1. Under the controller lock, `service_inputs` clears IMAN.IP and drains
   the event ring. It never waits.
2. For each completed transfer it copies the report out and re-arms the
   transfer.
3. Only after unlocking does the worker decode and publish into
   WovenInput: keyboard key-down edges (`keyboard_presses`, with shift and
   held-key suppression) and mouse motion and buttons
   (`decode_boot_mouse`).
4. It then sleeps in `wait_for_event` until the vector fires again.

USB input reaches the same queue as PS/2, so the kernel shell, `/bin/sh`
and the desktop all receive it.

**Two fixes found by this stage.**
- **MSI-X never delivered.** `msix::enable_owned` enabled the function with
  Function Mask asserted and never cleared it. Function Mask masks every
  vector, so no MSI-X interrupt could ever be delivered on any device. It
  is now cleared, and the write verified, as the last activation step.
- **The USB mouse Y axis was inverted.** HID boot mice report Y positive
  downwards, which is already WovenInput's orientation; only PS/2 reports
  up as positive.

## Stage 13.12 boot-time driver bring-up (2026-10-14)

Until this stage, every Stage 13 driver (`driver.rs`, `nvme.rs`, `ahci.rs`,
`xhci.rs`, `hda.rs`) was compiled only into its own test feature: a normal
boot contained none of them. `drivers.rs` now runs in every normal boot,
after storage and networking:
1. It registers a class matcher for NVMe, AHCI, xHCI and HDA with the
   WovenDriver manager.
2. For each driver, it finds the first PCI function the manager matches to
   it.
3. It binds that function through `driver::bind_pci`. Binding claims the
   function and its programmed MMIO BARs for the boot owner token.
4. Only then does it call the driver's `init_at(device)`, with the function
   re-read after admission because admission may move BARs.

Initialization failure unbinds again (`unbind_pci_resources`), so a failed
controller never keeps ownership. A missing controller is logged as absent.
Neither stops the boot. Ready drivers are added to the device registry
(`nvme0`, `ahci0`, `xhci0`, `hda0`) and listed by the shell's `devices`.

**Rules for this code:**
- A normal build compiles only the driver code it uses. The PCI hotplug
  functions stay gated to the Stage 13 tests.
- Each driver module carries one `expect(dead_code, reason = …)`, active
  only in production and in `stage13-12-test`. The reason names the APIs
  still waiting for production callers. The later Stage 13 closure steps
  (USB input, audio) remove it by giving those APIs callers.
- Shared PCI/paging items that the live binding path now uses had their
  dead-code expectations narrowed so they do not apply in those builds.

**Limits.**
- One function per driver: a second controller of the same class stays
  unbound.
- The controllers still poll synchronously under their own locks during
  initialization.
- Storage is not yet mounted from NVMe or AHCI.
- USB devices are not yet read, and audio is not yet exposed to userspace.

## Stage 13.4–13.7 revalidation (2026-10-14)

The ordered AHCI/SATA, xHCI core, USB HID, and WovenInput software gates pass
on 1/2/4 CPUs in the current tree. Their evidence is recorded in the
[Stage 13.4–13.7 validation audit](audit-stage13-4-7-validation-2026-10-14.md).
The QEMU fixtures validate bounded ownership and protocol contracts; they do
not substitute for native-controller or physical USB interoperability. USB
mouse report decoding is still outside the accepted keyboard-only fixture.

The bounded USB boot-mouse decoder now exists in `xhci.rs`; it emits WovenInput
motion and button-edge events from copied report bytes and carries button state
explicitly between reports. It is self-tested and does not yet claim a live
xHCI mouse polling worker or physical-device qualification. See the Stage 13.6
USB mouse decoder audit.

## Stage 15 bounded WovenFS VFS write-through (2026-10-14)

The `/wfs/<name>` namespace has one production mutation authority:
`task::open_current`, `task::write_current`, `task::mkdir_path`,
`task::rename_current`, and `task::unlink_current` route
through native WovenFS. Creation is durable before a read-only VFS node is
published; writes commit the native transaction before updating the VFS cache
or shared descriptor offset; unlink follows durable-first ordering. The
adapter validates nested native paths and rejects traversal, empty components,
NULs, and prefix confusion.

Native mutations serialize their complete read-modify-write operation with a
rank-5 `PreemptMutex`, below the ATA device lock at rank 10. No IRQ-disabled
guard spans device I/O. Directory creation, empty-directory removal, and
rename update parent entries transactionally; ATA publication recursively
imports validated nested directories with bounded depth and visited-inode
tracking; `Volume::check()` applies the same complete-tree integrity boundary.
Mounted system policy and physical flash qualification remain open. See the Stage 15
write-through audit.

## Stage 11.2 scheduler-backed thread ownership (2026-10-14)

ASLR geometry CPU regression (2026-10-16): the private graph-loader geometry
seam exercises supported production draws without changing the userspace ABI
or normal randomization. Stage `11.2-aslr` fills all sixteen 64 KiB mmap slots
before writing the prevalidated stack guard. Stack draw 16 / mmap draw 0
currently exits 99, proving the guard became writable; required fault -11 is
not met. This failing gate is evidence, not a planner correction or acceptance.
See the ASLR geometry audit. Both its call site and module are feature guarded.

Bounded main-image local-exec TLS accepted (2026-10-16): `InitialTls` records a checked
Variant-II layout and an independently tracked RO/NX post-relocation template.
Mutable main/child storage contains a mapped FS:0 self pointer; fresh threads
receive initialized template state, not mutated main-thread bytes. Parser
backing/congruence, clone, logical permission and teardown accounting are
updated together. Expanded compiler fork/cancel/capacity/owner-exit and biased
pointer-initializer execution passes on every online CPU in 1/2/4-CPU boots;
full session 37505 passed with 515 unchanged inputs, seventeen feature lints,
nine focused and twenty-one inherited boots, plus sixty stress logs. See the
implementation audit for scope: pure-BSS runtime and broader/library TLS remain
open, as does the separate production-ASLR arena-overlap finding. Later older
TLS-pending statements in this section describe the pre-correction baseline.

Compiler shared-PIE correction accepted (2026-10-15): ELF parsing explicitly
accepts a single inert null DT_DEBUG entry, rejecting nonzero addresses and
duplicates. The new opt-in compiler gate executes exact signed Rust/LLD output
through the graph loader before requiring every original Stage 11.5 check.
Session 73596 passed full host/lint and inherited 1/2/4-CPU preservation with
all 505 inputs unchanged. Compiler TLS remains open. See the compiler-PIE audit.

Accepted loader placement correction (2026-10-15): the validated ELF kind now
selects main-image placement. ET_EXEC ignores an offered ASLR delta; ET_DYN
retains it. The signed graph loader, dependency ownership and library windows
are preserved. A nonzero-candidate compiled static fixture and a biased PIC
probe pass on 1/2/4 CPUs. Full host/lint, exact Stage 10.7 and Stage 10.8–11.5
preservation passed in session 62865, exit 0, with all 500 inputs unchanged.
See the executable-placement audit and Claude's source policy `6f02202d`.
Compiler TLS remains open; this does not qualify a new release or full stage.

Accepted foundation/status corrections (2026-10-15): user-mapping ancestors
are explicitly writable while leaves retain W^X/RELRO/COW policy, including
read-only COW/file mappings; this prevents sole-owner COW refaults. Process wait
checks parent ownership before scheduler liveness, without nesting those locks,
and rechecks in the original reap transaction. Process wait and thread join
share a zero-extending 32-bit exit payload encoder, separate from 64-bit errors.
The real libwoven fixture catches these boundaries and preserves original
Stage 11.2 assertions. All three bounded corrections are accepted in prerequisite
order on the unchanged 500-input baseline, with full Stage 10.7/10.8–11.5
preservation on 1/2/4 CPUs and warning-denying host/fixture/kernel QA. See the
three dedicated audits and combined evidence. None is a release/hardware
qualification or complete compiler-TLS/loader interoperability.

Earlier-stage IPC follow-up (2026-10-15, bounded correction accepted): `HandleSpace`
now skips empty slots whose preserved 24-bit generation is already maximum.
The last live generation remains valid until close. The existing state guard,
16-slot ordinary capacity, 32-bit encoding, metadata and five installation
paths are unchanged. Actual-source boundary probes, production rollback/teardown
and inherited runtime preservation pass on the frozen baseline; see the
handle-generation audit. This is separate from thread-record retirement.
Durable host coverage is now part of `cargo test`; a `qemu-test`-guarded
post-SMP probe exercises real installation rollback, queued escrow, waiters,
lineage and frame cleanup. Its final revised full acceptance chain exited 0,
including full Stage 10.7 and Stage 10.8–11.5 matrices on 1/2/4 CPUs. All 488
frozen inputs matched at that increment's acceptance. Signed-status encoding
has since passed its own separate 500-input acceptance;
full runtime, release and hardware production closure remain open.

The registry now reserves generation `u32::MAX` as a permanent exhaustion
sentinel: a free record at that value is never allocated again. All three
release paths retain their existing checked generation advancement under the
rank-10 registry guard. The accepted bounded correction has five actual-source
host regressions plus full Stage 10.7 and Stage 10.8–11.5 preservation on
1/2/4 CPUs with unchanged frozen sources; see the thread-generation audit.
The exit-result ABI collision has since been corrected and accepted separately;
compiler-compatible TLS remains open.

`TaskControlBlock` carries both the stable process owner and an optional
generation-safe thread ID. User-thread creation maps a private NX stack and a
private initialization from the immutable main-image TLS template into the existing
address space; it never retains an unchecked userspace pointer. Thread exit
publishes registry termination only after the scheduler completes the stack
hand-off, so join can safely reclaim those mappings. Join waiters use the
scheduler's latched event path, and process termination retires ready/blocked
siblings immediately while running siblings receive a safe-point termination
request. `ThreadCancel` (ABI 136) applies the same two-state rule to an owned
sibling without killing its process: ready/blocked targets are marked dead and
the registry is transitioned under the scheduler lock, while running targets
carry the request through the existing post-switch finalizer. Both paths wake
the single join waiter and retain the generation-tagged record until join
reclaims the private mappings. The address space remains owned by the process
until its parent reaps the process. Process exit now retires unjoined sibling
threads, defers address-space destruction until all scheduler hand-offs are
complete, and reclaims generation-tagged thread records only after the parent
wait path releases the process-table lock. Shared libraries, symbol
resolution, dynamic TLS, and physical qualification remain outside this
bounded increment.

## Stage 11.5 bounded symbol relocation (2026-10-14; scope groundwork 2026-10-15)

`elf::DynamicImage` validates a SysV hash table, dynamic symbol table, and
string table within bounded file-backed load segments. `userspace::load_elf`
applies `R_X86_64_RELATIVE` and locally defined `R_X86_64_64`,
`R_X86_64_GLOB_DAT`, and `R_X86_64_JUMP_SLOT` entries before W^X/RELRO
finalization. The subsequent signed-library integration in
`userspace/linker.rs` loads a breadth-first graph of at most four verified WAPP
dependencies from `/lib`, checks SONAME identity, maps each into a separate
window, applies cross-image relocations, and protects every RELRO range.
`elf::SymbolScope` preserves discovery order and supports transitive cycles.
The process owns all mappings; an unpublished `Pending` guard rolls back all
failures, including argv setup. Compact page-range metadata retains forty
mapping slots within the existing fork stack budget. Root destruction walks
every empty private user-table branch and refuses copied kernel branches.
Legacy REL/PLT tables, versioned/weak symbols, constructors/destructors, lazy
binding and library TLS remain unsupported. See
`audit-stage11-shared-libraries-2026-10-15.md` for validation and limits.

The historical main-image TLS mapping was not a compiler-compatible TCB:
the real Rust local-exec probe expects a self-pointer at `FS:0` and aligned
template bytes immediately below it. The existing page-end FS base has neither;
new-thread cloning also copied mutable main-thread state. The correction and
full bounded compiler local-exec acceptance are recorded above and in the
compiler-TLS implementation audit. Do not infer broader TLS-model acceptance.

The symbol resolver and the production loader now share
`Image::defined_symbol_value`: ELF symbol values receive the image load bias
before validation against relocated segments, and the entire declared symbol
range must fit. Local relocation can reference private definitions; scope
exports require global binding, default/protected visibility and ordinary
NOTYPE/OBJECT/FUNC semantics. Undefined, TLS, IFUNC and reserved section values
cannot become ordinary relocation addresses. Dependency scope validation
preserves `DT_NEEDED` order, and resolution requires a complete validated scope.
Parser-backed regression fixtures live in `tests/elf_scope.rs`.

Debug builds optimize the `sha2` dependency (`profile.dev.package.sha2`) so
hashing the complete debug kernel ELF does not dominate emulated boot. Full
kernel measurement and the SHA known-answer check remain enabled.

## Stage 13.2 PCI power-state ownership (2026-10-13)

`hal::pci::suspend_owned` and `resume_owned` operate only on a validated,
generation-owned function. Suspend first disables and verifies I/O, memory,
and bus-master decoding. If the discovered PCI capability list contains Power
Management, the exact low-half PMCSR value is checkpointed and the function is
transitioned to D3hot; resume transitions the same capability to D0 before
restoring the command checkpoint. PMCSR writes preserve the upper half of the
containing configuration dword, preventing adjacent PM capability fields from
being clobbered. A failed transition attempts the safe rollback state and
returns an error, while the driver manager retains the checkpoint until the
binding state publication succeeds. Interrupt-active bindings cannot suspend.

This is a software ownership and configuration-state boundary. Device reset,
D3cold, platform wake signaling, and physical device suspend/resume still
require device-specific drivers and hardware qualification. See the
[Stage 13.2 suspend/resume audit](audit-stage13-2-suspend-resume-2026-10-13.md).

The PCI hotplug coordinator compares both BDF and the full driver-relevant
identity (vendor/device, revision, class, subclass, and programming
interface). A same-slot replacement detected before old-owner quiescence fails
closed before any teardown configuration write; a platform hotplug boundary
must establish ordering before the replacement can be processed. Unbound
persistent BAR reservations are released before topology invalidation;
release failures leave the removal retryable. See the [hotplug insertion audit](audit-stage13-2-hotplug-insertion-2026-10-13.md).

## Stage 13.2 PCI BAR admission lifetime (2026-10-13)

Persistent BAR reservation state is released only after BAR clear/read-back,
published-inventory refresh, and bridge-window rerouting succeed. The binding
remains in `Unbinding` when any phase fails, so no allocator range can be reused
while hardware or forwarding state is uncertain. See the [automatic BAR
admission audit](audit-stage13-2-bar-admission-2026-10-13.md).

## Stage 13.2 verified MSI activation (2026-10-13)

Owner-bound MSI activation keeps the vector and interrupt-mode claims private
until the capability header, message address/data, and final enable state all
read back as expected. The original capability image is retained with MSI
disabled during recovery; if restoration cannot be verified, the concrete
lease remains retained for ordered teardown. Disablement likewise verifies the
final disabled header before releasing mode and vector ownership. Physical
APIC delivery still requires hardware qualification. MSI-X activation follows
the same ownership boundary, asserting Function Mask before table MMIO and
retaining the lease if cleanup fails. See the [MSI rollback audit](audit-stage13-2-msi-rollback-2026-10-13.md).

The Stage 13.2 preservation probe keeps hardware qualification conditional:
it scans the discovered QEMU fixtures without creating shared mappings, and
only attempts generic interrupt activation when a capability and the required
BAR are admissible. Its disposable NVMe/e1000 fixtures exercise inventory and
fail-closed admission; the QEMU virtio fixture now qualifies MSI-X activation
and teardown, including the high-MMIO fallback mapping. Physical vector
delivery and PCIe hardware lifecycle qualification remain open.

## Stage 13.3 NVMe queue and failure lifecycle (2026-10-13)

The NVMe driver owns each queue, identify buffer, and transfer buffer through
an owning `DmaPage`; dropping any partially initialized controller therefore
returns its physical frames to the allocator. Initialization also snapshots the
PCI command register and uses a rollback guard that disables the controller
with bounded readiness polling before restoring the exact command value. This
keeps failed discovery from leaking DMA memory or leaving bus mastering active.

The accepted gate is limited to the disposable QEMU controller and synchronous
polling I/O. Interrupt completion, controller reset/recovery, hotplug removal,
multi-namespace policy, and physical NVMe qualification remain open. See the
[Stage 13.3 NVMe audit](audit-stage13-3-nvme-2026-10-13.md).

## Stage 13.8 HDA register addressing (2026-10-13)

HDA MMIO access splits each register address into its containing page and
offset before performing the volatile access. This is required for the reset,
command-ring, codec, stream, and mixer registers; mapping a page without
adding the offset silently aliases every register to the BAR base. The QEMU
HDA codec/topology/stream gate now passes on 1/2/4 CPUs. Physical codec and
interrupt/DMA recovery qualification remain open. See the
[HDA MMIO audit](audit-stage13-8-hda-mmio-2026-10-13.md).

## Stage 14 later-feature lint coverage (2026-10-13)

PCI persistent BAR admission diagnostics are guarded across the complete later
feature family, not only the Stage 13 feature set. This keeps `-D warnings`
meaningful when Stage 14–20 code compiles the shared PCI module. The live
Stage 14.1–14.5D network matrices pass on 1/2/4 CPUs; hardware NIC and radio
qualification remains separate. See the [Stage 14 feature-gate audit](audit-stage14-feature-gates-2026-10-13.md).

## TCP close/drain repair (2026-09-24)

A closed TCP descriptor retains its transport after the final async reference
until graceful close completes. `network::poll` reaps those existing bounded
slots; a 30-second grace expires on a subsequent poll for unresponsive peers.
Slots remain counted and unavailable for reuse until retirement. This fixes
queued-data loss when a send completion was consumed before transmission.
See [the TCP repair audit](audit-tcp-close-drain-2026-09-24.md) for failure
evidence, locking/ownership review and validation limits.

## Selected physical machine and inventory boot (2026-09-24)

The latest user instruction selects this HP Pavilion x360 with AX201
`8086:A0F0`, superseding the earlier AX200 target preference. Native AX201
transport is not implemented. The existing AX200 activation boundary remains
unchanged until the appropriate device-specific implementation is reviewed.

`physical-probe` is an independent root/kernel Cargo feature. After early
RAM and paging checks, `kernel_main` enters `physical_probe::run`, enumerates
PCI through the existing HAL, prints network identities and AX201 BAR/capability
details to the framebuffer and serial, and halts. It does not enter driver
activation, storage/network runtime, AP startup or candidate QEMU tests. No
device configuration or MMIO register writes are added by this mode; PCI
configuration reads use the existing ECAM/legacy address-selection access.
It is an inventory aid and never radio acceptance. See the
[AX201 machine audit](audit-ax201-machine-2026-09-24.md).

## AX201 integrated 22000 activation boundary (2026-10-17)

The AX201 owner increment adds `Ax201Driver` in `wifi_firmware.rs`. It is a
single lifecycle owner for the validated API-77 runtime image, copied
LMAC/UMAC/paging DMA, context-info DMA, startup state, ALIVE transition and
stop teardown. The firmware input is an explicit named provider source whose
filename must be `iwlwifi-QuZ-a0-hr-b0-77.ucode`; the external bytes are copied
before the source may be released. `Ax201QueueOwner` allocates the 22000 MQ
RX free/used/status rings, command ring and one RX buffer per 512-entry MQ
free descriptor,
and startup refuses to proceed without attaching it. The concrete adapter
programs the RFH free/used/status queue bases, resets their device cursors, and
rings the MQ RX shadow write-index PRPH doorbell at `0x1c80` before
`CPU_INIT_RUN`.
On reclaim, every device-produced AX201 used completion is checked for a
bounded rbid before any RX copy, preserving the DMA ownership boundary for
malformed completions and teardown. The free ring uses 16-byte transfer
descriptors; the used ring uses 32-byte completion descriptors.
The command ring uses the 22000 `iwl_tfh_tfd` 256-byte stride (25 TB slots plus
header/padding), rather than a legacy 128-byte descriptor assumption.
The context-info command-queue size is encoded as `TFD_QUEUE_CB_SIZE`
(`ilog2(entries) - 3`, `2` for the 32-entry ring); it is kept distinct from the ring's descriptor
count used for indexing and bounds checks.
The owner also exposes a bounded four-outstanding host-command path: it
copies the 4-byte short or 8-byte wide header and payload into kernel-owned
DMA, writes one 10-byte transfer-buffer entry in the 256-byte TFD, uses
command queue 9, and retains that payload until a matching response sequence
retires it. This is transport plumbing only; the physical probe sends no
speculative firmware command before the device's real ALIVE is observed.
The TFD is emitted by a shared checked encoder that bounds the 25 transfer
buffers, rejects zero lengths and addresses outside the 36-bit transport
range, and is covered by the queue-owner self-test before DMA publication.
The physical probe is invoked only after the normal IDT/APIC boundary is
initialized; it enables PCI memory/bus mastering, binds the identity-specific
AX201 MSI vector, enables CSR RX causes, services the deferred IRQ boundary,
and orders CSR/MSI disable before restoring the PCI command register.
Once ALIVE is observed, the physical probe uses the API-defined statistics
command (`0x9c`) as a bounded command/response smoke test, with periodic
notifications disabled. It reports that result separately from radio scan and
association qualification.
The firmware owner also exposes typed scan-control boundaries for the API
configuration and abort commands and for scan-complete notifications. These
encode/parse checks are strict about group, command, version, UID, and payload
size. It also emits the explicit API-17 passive UMAC request for at most 67
channels and validates the variable-length per-iteration result records.
Direct-SSID active probing remains fail-closed until probe-frame and MAC/PHY
context setup are implemented; these boundaries do not claim radio results.
The scan session owner retains one active UID, requires that UID for abort and
unsolicited completion handling, and clears it only on a matching completion.
This is the cancellation/generation boundary for the scan command path.
The AX201 boundary also exposes a checked API-1 PHY context encoder and
submission method. Its 40-byte packed layout validates channel, band, and
width before DMA publication. It is deliberately not part of the physical
probe sequence yet: MAC context, binding context, command status, and
quiescent teardown must precede radio-command activation.
The boundary now also contains the fixed managed-station MAC record and API-2
binding record. Physical activation submits those records only after ALIVE and
accepts them only through the generic zero-status response parser; timeouts,
unsolicited packets, and nonzero firmware status do not advance the sequence.
Once all three contexts acknowledge successfully, the physical path submits a
bounded passive scan on channels 1/6/11 and routes only UID-matching
iteration/completion notifications to the scan owner. A timeout attempts the
matching abort before teardown; active/direct-SSID probing and association are
separate later boundaries.
Physical startup reads the Intel `CSR_HW_REV` register at BAR offset `0x28`
for context-info `version.mac_id`; the PCI revision byte is not substituted.
It also reads the six-byte CSR MAC identity from `0x380/0x384` for the
activation diagnostic, while leaving radio/network qualification separate.
The inventory-only probe reports this register as well, but keeps its
read-only/no-DMA boundary.
`Intel22000MmioIo` accepts the 22000 24-bit PRPH address field
needed by AX201's `UREG_CPU_INIT_RUN` (`0x00a05c44`). Stage 13.10AE/AF/AG
passes in the Stage 13.9 1/2/4-CPU matrix. This is still synthetic: the owner
has no normal-boot attachment, physical ALIVE, queue-doorbell, interrupt,
recovery or radio evidence.

The Intel AX201 identity `8086:A0F0` shares the Intel 22000 CSR, DMA
context-info, firmware-startup and deferred-interrupt contracts already
implemented for the AX200-family path, but uses a separate integrated
firmware profile and MSI admission function. AX201 admission is therefore
explicit and cannot fall through the AX200 identity check. Firmware loading,
physical ALIVE proof, real queue activation, teardown/recovery and radio
qualification remain separate gates.

The AX201 owner now also has a distinct bounded data-TX ring: 32 hardware
TFD slots and up to four owned encoded payloads. A payload is published only
after its queue ID is checked against Intel's management (5–8) or data
(10–30) DQA ranges, and is
retained until an exact queue/index completion is decoded. The driver exposes
this boundary is now exercised by the physical probe's post-ALIVE DQA and
queue-allocation handshake. A tested management-frame submission still
requires an explicit frame/peer policy and returned hardware TX evidence.

## Physical Wi-Fi firmware prerequisite (2026-09-24)

The selected machine's AX201 PCI identity (`8086:A0F0`) is represented in the
Wi-Fi binding table as `ActivationReady`. Classification records the exact
family and integrated `iwlwifi-QuZ-a0-hr-b0` API-77 profile. The shared
22000 CSR path accepts AX201, while the AX201-specific MSI contract remains
identity-specific and cannot reuse the AX200 binding by accident. No firmware
blob is embedded or loaded; native firmware transfer, DMA, interrupt,
teardown and RF work remain prerequisites for physical activation.
`wifi_ax200_image.rs` preflights runtime LMAC/UMAC/paging region ordering,
payload size and total DMA capacity. `Intel22000DmaContextInfo::from_runtime_firmware`
uses that plan to copy payloads into independently owned DMA buffers. Failure
drops the unpublished partial context; success still requires queue setup and
a reviewed physical publication/lifecycle owner. See the
[runtime-loader audit](audit-ax200-runtime-loader-2026-09-24.md).

`wifi_firmware.rs` delegates Intel container parsing to `wifi_tlv.rs`, an
allocation-free, immutable-borrow parser shared with host tests. It validates
a bounded complete blob, exposes paging-size metadata separately, and
streams runtime/init SEC records including secure variants and explicit
CPU/paging separators. The DMA stager rejects separators before allocation.
This closes real-container parsing failures; it does not implement the
physical transport. The inspected host is AX201, while the candidate match
is AX200. See [the physical preflight audit](audit-physical-wifi-preflight-2026-09-24.md)
for firmware hashes, validation and the remaining lifecycle/transport work.

## Stage 13.10AC integration boundary (2026-09-23)

The sections below describe previously recorded foundations. The current
Wi-Fi candidate is additional, feature-gated source: `main.rs` includes
`wifi_hw` and the other Wi-Fi modules only with `stage13-9-test`. It does not
yet provide the normal boot path with a working physical AX200 driver.
The session and smoltcp adapter now share that boundary. `network.rs` keeps
the candidate transport selector in a feature-gated module, while ordinary
builds use `VirtioSmolDevice` directly. This corrects the previously inconsistent
module graph without changing the VirtIO packet path.

`interrupts.rs` installs the reserved PCI Wi-Fi vector `0xd0`. Its hard IRQ
publishes an atomic work flag and acknowledges the LAPIC. `wifi_hw.rs` exposes
`service_deferred_ax200_interrupt`, which consumes that flag and calls
`IntelRxInterruptController::service` outside the hard IRQ. The controller
masks CSR delivery, reads interrupt status, acknowledges enabled RX causes,
and restores its configured mask; fatal hardware/firmware status returns an
error with delivery masked. The flag coalesces notifications and is not a
counted queue or a scheduler wakeup. The only current deferred-service caller
is the synthetic self-test. A production worker, device lifecycle ownership,
teardown/recovery, and physical interrupt/DMA qualification remain open.

The synthetic CSR test uses an owned DMA page and verifies written values;
ordinary RAM does not emulate write-one-to-clear hardware semantics. The
MSI discovery branch explicitly skips physical programming when AX200 is
absent. Neither synthetic success nor that skip proves physical Wi-Fi works.

`scripts/test-stage10-runtime.py` now pins all 54 Wi-Fi acceptance markers
through 13.10AC in addition to the SMP and async ABI markers and exit code 33.
`tests/test_runtime_harness.py` checks complete evidence, each missing Wi-Fi
marker, the former incomplete marker set, and unsuccessful QEMU exit.
The Stage 13.9 launcher uses the same project-local Cargo directories as
the Stage 10.7 preservation launcher.

## Candidate feature boundaries (2026-09-24)

Ordinary builds retain the VirtIO network transport, PCI discovery and legacy
I/O bus-master path, the reserved Wi-Fi interrupt vector, and PS/2 keyboard
input. Candidate-only APIs are compiled alongside their existing callers:

- The Wi-Fi secure-pool candidate and MSI programming module use
  `stage13-9-test`; the normal best-effort network RNG is unchanged. The
  deterministic test pool is not a production cryptographic entropy source.
- PCI MMIO bus-master enablement follows the existing NVMe, AHCI, xHCI, HDA,
  and Wi-Fi candidate features. Two uncalled private configuration-write
  wrappers were removed; driver-used configuration transactions are retained.
- Audio registration uses `stage13-8-test`, matching `woven_audio` and HDA.
- Extended input event candidates and their self-test use `stage13-7-test`.
  No non-keyboard producer exists in the ordinary build. Its byte input ABI,
  bounded queue, overflow accounting, and IRQ-safe locking are unchanged.
- Wi-Fi deferred-work consumers and BSP MSI destination selection follow the
  Wi-Fi candidate feature. The installed interrupt handler is unchanged.

These boundaries do not supply missing production drivers. They add no
syscalls, capabilities, kernel objects, locks, or asynchronous ownership paths.
Existing candidate acceptance tests remain enabled under their original
features; no new lint suppression was introduced.

The Stage 13.6 HID report-error branch now reports the HID error directly;
a duplicated HDA discovery block was removed. Stage 13.8 retains its original
audio codec/topology checks. The normal-release shell harness isolates the
bootloader dependency's nested Cargo installer in `target/bootloader`, while
an explicit parent `--target-dir` keeps the main artifact tree unchanged.
This avoids the parent/child release artifact-lock deadlock. The validated
results and physical-driver limits are recorded in the continuation audit.

## Scope and source of truth

This guide describes the Stage 10.7 source, not the proposed 1.0 system. WovenHat
currently has a monolithic Rust kernel, a UEFI boot image builder, embedded Ring-3
programs, and host/QEMU acceptance harnesses. The supplied
[master development roadmap](master-development-roadmap.md) defines future stages.

## Stage 1–5 data journal boundary

FAT32 persistence retains the original single-file `WDJ1` journal and adds a
separate bounded `WDJ2` batch journal. A batch contains at most sixteen file data
replacements. Prepared records carry the old bytes and committed records carry
the intended lengths/checksums; WMD1/WMD2 metadata intents are published before
data and removed or finalized with the batch. Mount recovery rolls back
incomplete or partially matching batches, removes stale metadata intents, and
retires only a fully matching commit. The batch API is
`storage::persist_paths_atomic`. This is bounded data-plus-sidecar recovery,
not a general multi-operation metadata transaction or physical power-loss
qualification.
[Stage status](stage-status.md) records which gates have actually passed.

## Repository map

| Files | Responsibility |
| --- | --- |
| `Cargo.toml`, `kernel/Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml` | Workspace, pinned toolchain, freestanding kernel artifact and feature selection |
| `build.rs`, `src/main.rs` | Package the kernel with the UEFI bootloader; expose the generated image path to harnesses |
| `kernel/src/main.rs`, `config.rs` | Initialization order, subsystem wiring, configuration bounds and boot acceptance orchestration |
| `hal/`, `ap_start.S`, `smp.rs`, `gdt.rs`, `interrupts.rs`, `pic.rs`, `timer.rs` | CPU/platform discovery, AP startup, descriptor tables, interrupts and timekeeping |
| `task.rs`, `userspace.rs`, `elf.rs`, `syscall.rs` | Scheduling, process lifecycle, Ring-3 images, ELF validation, scheduler-checked static kernel-stack canaries, and syscall dispatch |
| `memory.rs`, `paging.rs`, `heap.rs`, `swap.rs`, `file_frames.rs`, `file_mapping.rs`, `page_cache.rs` | Physical/virtual memory, allocation, backing storage and file mappings |
| `block.rs`, `block_cache.rs`, `block_io.rs`, `ata.rs`, `partition.rs`, `gpt.rs`, `storage.rs` | Block devices, request execution, caching, partitions and storage initialization |
| `vfs.rs`, `fat32.rs`, `async_file.rs` | Filesystem operations and positional asynchronous file requests |
| `async_op.rs`, `async_network.rs`, `irq_lock.rs` | Completion ownership, asynchronous sockets and interrupt-safe network/completion locks |
| `network.rs`, `virtio_net.rs` | smoltcp IPv4 sockets and the transitional VirtIO-net transport |
| `capability.rs`, `wovenguard.rs`, `audit.rs`, `permission.rs`, `secret_vault.rs`, `signed_app.rs`, `secure_boot.rs`, `boot_measurement.rs`, `entropy.rs` | Authority, domains, resource policy, runtime permission decisions, volatile WovenVault handles, signed applications, software signed-kernel handoff, boot measurement, security-ledger recording/read ABI, and entropy support |
| `ipc.rs`, `pipe.rs` | IPC objects, endpoint/message operations and byte pipes |
| `device.rs`, `keyboard.rs`, `mouse.rs`, `serial.rs`, `console.rs`, `terminal.rs`, `shell.rs` | Device registry, input (keyboard and PS/2 mouse both decode and publish at IRQ time), diagnostics and command interfaces |
| `graphics.rs`, `gui.rs` | Kernel colour definitions and legacy framebuffer/GUI support; the userspace WovenGraphics abstraction lives in `libwoven::graphics` and feeds the existing compositor surface ABI. The userspace renderer also exposes bounded rounded panels, alpha-aware image blits, `Path` geometry with clipped opaque/alpha strokes and fills plus four-segment quadratic flattening, bounded eight-contour `CompoundPath` even-odd fills and strokes with integer 4x4 antialiasing, clipped vertical and horizontal gradients, deterministic signed-16.16 affine `Transform` values for clipped transformed rectangles, a shared-glyph `Font`/`TextMetrics` provider for bounded layout, wrapping and integer scaling, and a borrowed `TrueTypeFace` metadata/cmap/advance-metrics boundary with BMP format-4 and non-BMP format-12 lookup, bounded `fvar` axis metadata, validated borrowed `gvar` glyph-payload slices, simple and XY/transform/point-matched composite `glyf`/`loca` outlines, compound-path assembly, antialiased glyph raster entry point, advance-based text layout with optional legacy kerning, bounded character and word-level text-box line breaking, explicit LTR/RTL placement and shared bitmap fallback; `gvar` tuple delta application, hint execution, modern shaping, bidi and scripts remain open |
| `rtc.rs` | CMOS real-time clock (UTC), behind the `ClockRead` syscall |
| `font.rs`, `display.rs` | 5×7 ASCII font (also compiled into libwoven); `Surface` (opaque `blit`, `blit_region` for exposed-region redraw, uniform-opacity `blend`, `write_pixels` for client uploads), bounded `Damage` (regions that form exactly a rectangle merge; a full list collapses to one bounding box) and the single format-resolving presentation path (`present_scaled`, integer display scale, each row converted once and copied to the physical rows it covers; `present` is scale 1; `Surface::scaled_row` serves the virtio-gpu heads) |
| `user_graphics.rs`, `compositor.rs` | Owner-tagged, generation-checked client surfaces (syscalls 98–101, 104), up to 1920×1200 within a 4 MB per-process and 16 MB system pixel budget, behind a preemption lock (not interrupts-off) with allocation and freeing outside it and the kernel-held compositor surface they are blitted onto (102–103); `blit` snapshots under the registry lock and releases it before taking the compositor lock, so the two rank-10 locks never nest. Presentation borrows the framebuffer through `terminal::with_framebuffer` (lock order compositor → terminal), and display ownership suppresses terminal rendering while the compositor presents. Userspace reaches the compositor only with `DisplayIo` and the single-holder lease (syscalls 105–106), released on exit; every client token is bound to the calling process before the registry sees it. A presenting holder also takes input focus: `woven_input` routes events to its private bounded queue (syscall 107, `InputIo`) until it exits; syscall 108 sleeps for the next event with a single registered waiter woken by `publish` after `FOCUS` is released (lock order scheduler → compositor → FOCUS). A client may share one of its surfaces with one process (syscall 109); the lease holder composites it with syscall 110, authorised by the slot's grant rather than the token. Syscall 111 sleeps over input and the pid mailbox at once; `ipc::send` wakes the mailbox's single waiter after releasing the IPC lock. Clients declare damage on their surfaces (syscall 112); the holder sleeps on it via `WaitAny` and copies only the damaged region (syscall 113). The boot compositor is sized to the framebuffer (clamped to 1920×1200), and every framebuffer writer, including the boot `Console`, goes through the terminal lock. Stacking, raise and per-window focus are window-manager policy in userspace (Stage 19.11), built on the lease, focus, `WaitAny`, shared surfaces and IPC |
| `panic.rs`, `benchmark.rs` | Failure reporting and existing benchmark support |
| `libwoven/`, `apps/hello`, `apps/hello-gui`, `apps/woven-shell` | The userspace API (one thin wrapper per syscall, plus the typed `graphics::{Color, Rect, Renderer}` WovenGraphics boundary, `Canvas` software rendering, caller-owned `DepthBuffer`/`Vertex3` software 3D triangles, the shared kernel font, and a fixed-point `Spring` for animation, and `accounts`, local accounts held as a salt and `SHA-256(salt || PIN)`) and the Ring-3 programs. Each app is its own crate with a large-code-model `link.x` (whose `.l*` sections must be placed explicitly) and a `build.rs` that relinks when it changes. `woven-shell` is the desktop, with four workspaces (per-workspace stacks, a taskbar switcher, F3/F4): it holds the compositor lease and recomposes exposed rectangles from its own software layers and clients' shared surfaces (region blits, syscalls 120–121), hosting applications over an IPC protocol (`WOPN`, `TRAY`, `NOTE`, `WCLS`, and `WSIZ`/`WRSZ` for resizing; `libwoven::layout` tiles them); shadows and other translucent layers use region blends (syscalls 200–201), and text, borders and the pointer are overlays copied with a keyed blit that skips the transparent key (syscall 202), so nothing is read back from the screen. With `--accounts` it starts at a sign-in screen, and its lock screen asks for the signed-in account's PIN. A user stack is 8 KiB (`UserStack::SIZE`), so the shell keeps its tile buffer and desktop state in statics (.bss), never in `main`'s frame. The kernel's feature-gated VirtIO-GPU test path separately uses a bounded `VirglStream` encoder for checked VIRGL command framing; it is transport validation only until a visible GPU frame is accepted |
| `tests/`, `scripts/`, `run-stage*-acceptance.ps1` | Host regressions, QEMU orchestration and stage preservation gates |

## Stage 6 filesystem concurrency boundary

`vfs.rs` keeps node metadata and shared open descriptions in separate rank-10
IRQ mutexes; when both are needed, open descriptions precede nodes. Handles
carry non-wrapping slot epochs, and node references carry generations. Disk-
backed reads and lazy materialization snapshot identity, backing path, version,
and (for shared reads) offset, drop both VFS guards for ATA I/O, then revalidate
before publishing data or changing the seek position. A conflict retries a
bounded number of times. Rename increments node versions. Prefix iteration
copies bounded path names before calling external code. File-frame cache
updates may nest under the VFS guards at rank 10.

The global heap's metadata guard is rank 50 and disables local interrupts
during allocation and deallocation. Boot-time heap page mapping occurs before
acquiring that guard, preserving the paging (10) to heap (50) order. Its eager
mapped size is one sixteenth of available physical memory, clamped to 256 KiB
through 8 MiB. The allocator retains alignment padding with each allocation
and coalesces freed intervals. Kernel range mapping rolls back newly mapped
pages on a partial failure. Runtime growth maps 256 KiB chunks only after
releasing the heap guard; the boundary is published under that guard after
mapping succeeds. A rank-free, IRQ-enabled caller can grow synchronously on
allocation failure. BSP idle maintains mapped headroom when free space falls
below 1 MiB so guarded callers usually stay on the allocation-only path.
The growth window remains in the P4 slot already shared by user address
spaces. See the [growth audit](audit-stage6-heap-growth-2026-09-17.md).
The rank-40 physical-frame allocator stores one allocation bitmap in reserved
pages at the start of each usable RAM range. Returned frames beyond its 4,096
entry hot cache remain reusable through a bitmap scan; the bit also rejects
duplicate returns. Bitmap pages are excluded from allocatable-frame counts.
The heap now stores each live allocation's span in a header before its
payload, and stores free-list links inside freed spans. Its rank-50 guard
serializes those metadata writes; no fixed live-object or free-interval table
remains. The current mapped-byte ceiling and linear free-list search remain
separate limits.
The swap-state guard is rank 40 and releases before ATA transfers. Device,
keyboard decoder, journal-intent, mount-record, and key-vault metadata guards
are rank 10; none performs a blocking transfer while held. Keyboard captures
the caller's interrupt state before taking its guard to distinguish normal
IRQ-driven input from early-boot legacy polling.

The clean FAT32 page cache uses a rank-20 IRQ mutex for resident-page lookup,
copying, publication, and invalidation. A miss loads the 4 KiB page after
dropping that guard. Invalidation advances an epoch; a load that spans an
invalidation returns an I/O error instead of republishing stale cache data.

The shell's current-directory state is a short rank-10 IRQ-mutex section. It
copies the path into a fixed buffer before allocation or console output. The
global framebuffer terminal instead uses a rank-10 preemption mutex: it
prevents a local task switch while held but leaves device interrupts live
during long scroll and clear operations. Its rank-tracker updates briefly
mask local interrupts so an IRQ cannot observe a partial held-lock stack.
The terminal is never called from an IRQ handler and must not block or yield
while its rendering guard is held. Syscall writes use `file_fault_io` to
restore live interrupts around rendering even when syscall entry had IF=0.

This lock split covers the VFS/ATA slow-I/O boundary. FAT32 mutation still
crosses the storage and VFS layers without one transaction, so unrestricted
concurrent filesystem operations remain a separate production requirement.

## Stage 6 CPU lifecycle

Logical CPU slots and APIC identities remain stable while the online mask may
have holes. `online_count` is a population count, not an upper bound on valid
CPU indices. Scheduler placement uses a separate schedulable mask that excludes
APs draining or rebuilding their idle task; TLB shootdown snapshots the
physical online mask under the same transition lock that publishes an AP's
offline/online state. An offline request first plans all Ready-task moves under
the scheduler lock and commits them only if every task has a legal destination.
The CPU-owned idle checkpoint revalidates that plan before parking. A parked
AP retains its bootstrap stack and rejoins its original logical slot.
The requester cancels only a request the AP has not claimed; claimed offline
and online transitions use separate states and must reach a terminal result.
A rejected AP transition is rearmed for retry after AP cleanup. If a claimed
transition never completes, further hotplug requests remain disabled rather
than proceeding with uncertain CPU ownership.
The 4-CPU QEMU gate offlines CPU 1 while CPU 3 remains active, runs a worker on
CPU 3, and performs an acknowledged shootdown across the resulting hole.

## Stage 10.7 objects and data flow

`async_op::Handle` identifies a slot and a 32-bit generation. Its raw ABI uses
bits 0..15 for the slot, bits 32..63 for generation, and requires reserved bits
16..31 to be zero. A `Completion` is 16 bytes: signed status, reserved word and
64-bit value. The operation table records `TaskId` ownership, class, state and a
wait registration. Today ownership is task-bound; a shared per-process completion
port is Stage 10.8 work, not an existing property.

`network::UserSocket` associates a process owner and descriptor slot with a
smoltcp socket, peer, generation, async reference count and closing flag.
`SocketToken` carries slot, generation and owner for in-flight kernel operations.
Closing a pinned descriptor hides it from further descriptor operations; the
socket remains available to its existing tokens until the last pin is released.

`async_network::Request` stores owner, request identity, socket token, operation,
endpoint, length, kernel-owned bytes, result and generic completion handle.
The bounded queue holds pending, in-progress and completed requests. The worker
copies one request under the queue lock and releases that lock before entering
the socket runtime. Cancellation/teardown detach an in-progress request rather
than freeing its socket under the executing worker. The worker subsequently
releases that pin on completion or retry.

The submission path is:

1. `syscall.rs` checks WovenGuard network-device authority and validates arguments.
2. Send copies bytes from userspace immediately; receive retains only capacity.
3. `async_network.rs` pins the socket and allocates an owner-bound Network handle.
4. Queue publication precedes signaling the worker's latched scheduler event.
5. The worker attempts each queue slot at most once per pass. `WouldBlock` leaves
   that request pending and permits later requests to run. After the bounded
   pass, the worker waits for an event.
6. `network::poll()` drives smoltcp, releases its runtime lock, then notifies the
   worker if one locked queue snapshot sees pending or in-progress work.
7. Completion publication wakes a registered owner. Poll/wait copy completion
   and receive bytes to validated userspace destinations before consuming the
   request, releasing its socket pin and releasing the generic handle.

Failed copyout preserves the kernel result for retry. No asynchronous request
retains a userspace pointer. Cancellation does not roll back bytes already
accepted by the socket transport.

## Existing syscall boundary

| Number | API | Behavior |
| --- | --- | --- |
| 67 | Async cancellation | Owner-authorized cancellation; class-specific request cleanup |
| 77 | `AsyncNetSend` | Socket descriptor, input pointer and length; returns a completion handle |
| 78 | `AsyncNetRecv` | Socket descriptor and capacity; returns a completion handle |
| 79 | `AsyncNetPoll` | Nonblocking completion/data collection |
| 80 | `AsyncNetWait` | Event-driven wait followed by retry-safe collection |
| 81 | `AsyncTcpConnect` | Socket descriptor and packed endpoint; completes on connection readiness |

The audit corrections add no syscall numbers or capabilities. Existing network
policy gates remain at submission and collection; ownership checks remain on
handles. Teardown does not depend on retaining network permission.

## Concurrency and lock order

`irq_lock::IrqMutex` disables local maskable interrupts before taking its spin
mutex. Its guard releases the mutex before restoring the original interrupt
state and cannot be transferred to another thread. Nested guards preserve IF=0.
This addresses same-CPU preemption deadlock, which a plain spin mutex cannot
prevent even when the workload runs on only one CPU.

`irq_lock::PreemptMutex` is reserved for CPU-only sections that need live
interrupts. It increments a per-CPU no-preempt depth before acquiring the
spin mutex, and its guard releases the mutex and rank token before decrementing
that depth. The timer preemption path checks both this depth and the separate
I/O depth. It cannot be used in an IRQ handler or around a voluntary switch.

The network request queue, worker identity, generic completion table, socket
runtime and VirtIO-net transport use this guard. The runtime is rank 20 and
the transport is rank 30, matching runtime-to-transport packet flow. No task
may sleep or switch while holding it. The worker drops queue/runtime guards
before generic completion
publication or scheduler event waiting. Teardown can follow scheduler -> request
queue -> socket runtime. Network polling drops runtime before querying the queue;
transport polling releases its lock before socket-runtime acquisition. Runtime
packet handling can take runtime -> transport.

The socket worker remains CPU0-pinned. These changes do not introduce parallel
smoltcp workers, per-CPU network queues or unrestricted multicore I/O services.
The existing network runtime remains globally serialized. The guard does not
prove every cross-subsystem lock order; broader path audits remain necessary.

## Tests and evidence

`tests/async_network.rs` includes the production worker with deterministic host
socket/scheduler doubles. It covers blocked-before-ready requests, bounded passes,
parking, in-progress readiness notification, cancellation and racing owner cleanup.
`tests/irq_lock.rs` includes the production guard with host interrupt/mutex doubles.
`tests/test_tcp_harness.py` verifies host-server success and failure reporting.

The Ring-3 TCP probe queues receive before send on its second connection, forcing
the worker to handle a blocked operation without starving the send that enables
the host reply. It also checks invalid completion/data destinations and retries,
close while pinned, EOF, cancellation and owner teardown. The host requires both
TCP exchanges, required serial markers and QEMU debug-exit status 33.

`RUN-STAGE10.7.ps1` captures the chained acceptance transcript and snapshots QEMU
logs under `audit-artifacts/`. Stage 7.6 stress logs are retained per invocation.
`build.rs` tracks the actual kernel artifact so writing a log does not itself
trigger image reconstruction. Kernel changes still invalidate the image.

## Boundaries before later stages

Network progress still depends on the existing polling driver, not a new hardware
interrupt-driven NIC architecture. Send completion means local socket acceptance,
not remote acknowledgement; closing does not promise graceful draining. Public
socket descriptors remain process-local integer slots, while in-flight tokens
carry generations. Generation counters remain finite. Generic completions have
single-task ownership. These are
explicit architectural boundaries, not claims of a finished production network stack.

## Stage 10.8 completion ports

`completion_queue.rs` is the allocation-free reservation/FIFO core shared with
host tests. `completion_port.rs` adds process ownership, quotas, generation-tagged
handles, waiter registration and retry-safe batch claims. `async_op.rs` publishes
completion or cancellation into an associated port. Association and completion
serialize on the operation table before locking a port; scheduler notification
occurs after both are unlocked.

Syscalls 82 through 86 create, close, associate, poll and wait. See the
[ABI contract](completion-port-abi.md) for record layout, timeout and capacity
semantics. Port dequeue does not consume the original operation's data result.
`task::wait_for_event_until` extends the existing scheduler event latch with an
absolute deadline and timer-driven wakeup. Normal and forced process exit reclaim
ports after detaching owned operations.

`tests/async_runtime.rs` exercises production operation and port state with host
scheduler/lock doubles. `stage10_8.S` tests the real Ring-3 ABI;
`async_acceptance.rs` exercises multicore producers and a waiting consumer.
`RUN-STAGE10.8.ps1` preserves Stage 10.7 and adds 1/2/4-CPU completion-port boots.
Timers, event objects, IPC completion integration and per-thread cleanup remain
later roadmap work.

## Stage 10.9 timers and events

`async_events.rs` owns bounded generation-tagged timer and manual-reset event
slots. Timer deadlines use the monotonic tick source and are pumped by an
event-driven worker. Syscalls 87–94 expose create, wait, set, close and
sleep-until operations; timer and event waits produce the same generic async
completion records as other operations. Owner teardown cancels outstanding
waiters and releases every slot.

## Stage 13.10 Bluetooth architecture

Roadmap Stage 13.10 owns Bluetooth. Historical source/audit labels 13.10A–AC associated with AX200 are retained as historical Wi-Fi substage names under roadmap Stage 13.9 and must not be interpreted as Bluetooth milestones.

Stage 13.10A introduces `bluetooth_hci.rs` as a hardware-independent HCI protocol boundary. It owns bounded command packet construction, Command Complete parsing, command-credit state, opcode matching, and controller-status validation. The module is feature-gated by `stage13-10-test` and deliberately does not depend on xHCI, allowing the HCI state machine to be validated without pretending a physical transport exists.

The next layer, Stage 13.10B, should place a USB Bluetooth HCI transport beneath this protocol boundary using the existing xHCI core. Transport ownership must keep USB control transfers for HCI commands distinct from interrupt-IN HCI events and bulk ACL traffic. Hardware discovery, endpoint ownership, teardown, and DMA lifetime belong to the transport layer rather than the protocol parser. Later Bluetooth discovery, L2CAP, pairing/security, ATT/GATT and physical qualification remain above or beyond that boundary.


### Stage 13.10B USB HCI transport contract

Stage 13.10B extends the xHCI layer with Bluetooth USB descriptor semantics while preserving `bluetooth_hci.rs` as the transport-independent protocol boundary. The transport recognizes interface class/subclass/protocol E0/01/01 and requires three endpoint roles before accepting an interface: interrupt-IN for HCI events, bulk-IN for controller-to-host ACL data, and bulk-OUT for host-to-controller ACL data. HCI commands use the Bluetooth USB class control-request setup contract on endpoint zero.

The accepted 13.10B boundary parses and validates those roles and the command setup contract; it does not yet allocate dedicated Bluetooth transfer rings or claim live controller traffic. The next transport layer must own endpoint contexts, rings, DMA buffers, completion routing, bounded transfer sizes, teardown, and recovery. HCI event bytes should then be delivered upward to the protocol parser rather than interpreted inside xHCI.


### Stage 13.10C HCI transaction and transfer-execution boundary

`HciTransaction` is deliberately non-copyable mutable controller state. It owns the one-command-at-a-time credit/opcode contract: command bytes are encoded only after a credit is consumed, a second command is rejected while the transaction is outstanding, and readiness returns only after a matching successful Command Complete event.

The xHCI Bluetooth boundary maps the Stage 13.10B descriptor result into bounded transfer plans for endpoint-zero HCI commands, interrupt-IN HCI events, and bulk ACL input/output. These plans describe ownership and size constraints; they do not themselves claim that a USB transfer was executed. The next implementation slice must materialize the plans as dedicated xHCI endpoint contexts, transfer rings, DMA buffers and completion routing, then feed received HCI event bytes back into `HciTransaction`.


### Stage 13.10D live USB HCI mechanics and initialization

Stage 13.10D turns the accepted transfer descriptions into xHCI-owned execution state. The controller owns separate Bluetooth event, ACL-IN and ACL-OUT rings and DMA pages rather than reusing HID ownership. USB endpoint addresses are converted to direction-aware xHCI DCIs, and endpoint contexts distinguish interrupt-IN, bulk-IN and bulk-OUT endpoint types. HCI commands use an endpoint-zero control-OUT-with-data path; HCI events use interrupt-IN; ACL traffic uses the dedicated bulk rings.

The protocol layer remains independent of USB. `ControllerInitializer` composes `HciTransaction` into a bounded startup sequence: Reset is issued first, its matching successful Command Complete advances to Read Local Version, and only the matching successful version completion advances the controller to Ready. Transport bytes therefore cross into HCI state only at the protocol boundary; xHCI does not interpret controller policy.

The Stage 13.10D CI acceptance is deterministic and does not imply physical-radio qualification. A real Bluetooth USB controller must still validate endpoint behavior, timing, teardown/recovery and hardware-specific interoperability. Higher layers should build discovery/scanning and capability parsing above this accepted initialization boundary rather than adding policy to xHCI.


### Stage 13.10E discovery and controller capabilities

Stage 13.10E keeps discovery policy in the hardware-independent HCI layer. `DiscoveryState` owns a bounded 16-device BR/EDR Inquiry result set, deduplicated by Bluetooth device address. Inquiry event parsing validates the HCI parameter length before reading response records and stores only the fields needed by later link establishment: address, page-scan repetition mode, class-of-device and clock offset. xHCI remains responsible only for moving event bytes.

Controller capability parsing follows the same boundary. Read Local Version, Read BD_ADDR and Read Local Supported Commands Command Complete events are checked for the expected opcode, successful controller status and complete return payload before typed data is exposed. This prevents later connection/security layers from inferring controller support from transport presence alone.

The accepted 13.10E boundary is classic BR/EDR discovery plus local-controller capability discovery. It deliberately does not fold BLE scanning, connection establishment, pairing, L2CAP, ATT/GATT or policy into the transport. The next link layer should consume `DiscoveredDevice` records and capability data while retaining bounded handle/state ownership.


### Stage 13.10F ACL connection and ownership lifecycle

The Bluetooth HCI layer now owns the mapping from a discovered BR/EDR peer to a live ACL controller handle. `LinkState` is bounded to eight links and treats the controller handle plus Bluetooth address as an ownership pair. Replayed identical Connection Complete events are harmless, while a handle reused for another address or an address appearing under another live handle is rejected rather than silently aliasing authority.

`AclPacket` owns HCI ACL framing independently of xHCI. It masks the 12-bit connection handle, carries packet-boundary and broadcast flags, applies a bounded 1024-byte software payload ceiling, and validates declared lengths before exposing payload bytes. `LinkState::outbound_acl` and `inbound_acl` require the handle to remain live; Disconnection Complete removes the link before further ACL traffic can be accepted.

This boundary is intentionally below L2CAP. Stage 13.10G should parse/build the L2CAP basic header and add bounded channel identifiers/state while continuing to use the 13.10F ACL handle as the lower-layer authority anchor.


### Stage 13.10G L2CAP framing and channel ownership

L2CAP is layered above the 13.10F ACL authority boundary rather than owning transport handles itself. `L2capFrame` validates the Basic Mode length/CID header and uses fixed-capacity payload storage. `L2capChannels` owns at most eight established dynamic channels and records the ACL handle, PSM, local CID and remote CID for each.

Signaling establishes and tears down channel authority. Data transmission is permitted only through an established channel and is encoded for its remote CID; receive dispatch requires the same live ACL handle plus the channel's local CID. Removing the channel immediately makes both transmit and receive paths fail closed. This keeps higher Bluetooth protocols from treating CIDs as ambient/global authority.

Stage 13.10H should consume these owned channels when adding authentication/pairing and encryption state. It must not weaken the lower-layer ACL/CID ownership checks.


### Stage 13.10H Bluetooth trust authority

Bluetooth security is modeled as authority derived from controller-confirmed state, not inferred from connection existence. `LinkSecurityState` tracks authentication/encryption progression per owned ACL handle. `LinkKeyStore` owns a bounded peer-address-to-link-key association and answers controller Link Key Requests with a stored key or an explicit negative reply.

Secure Simple Pairing events are parsed into policy decisions: IO capability can be declared, but numeric confirmation and passkey acceptance require an explicit caller decision. The kernel does not silently accept a pairing prompt.

The strongest software trust predicate binds four facts: the ACL handle is still live, its peer address owns a stored link key, authentication completed successfully, and encryption was confirmed enabled by the controller. Key removal or ACL teardown therefore revokes trusted authority without relying on stale security state.


### Stage 13.10I Bluetooth LE discovery and link authority

Bluetooth LE is modeled with state distinct from the BR/EDR discovery and link-key trust paths. `LeDiscoveryState` owns a fixed-capacity table keyed by address type plus device address, because an LE peer identity cannot safely be reduced to the six address bytes alone. Advertising reports are accepted only when the LE Meta Event framing, subevent, declared lengths, address type and legacy advertising-data bounds are internally consistent.

`LeLinkState` is the LE controller-handle authority boundary. LE Create Connection consumes an owned discovery record, carrying its peer address type and address into the controller command. A successful LE Connection Complete establishes a bounded handle/role/address/connection-parameter record. Conflicting handle or peer ownership is rejected rather than aliased. Disconnection Complete removes the record and makes the handle stale immediately.

This layer intentionally does not reuse BR/EDR `LinkKeyStore` as BLE bonding state. Stage 13.10J ATT/GATT should consume live `LeLinkState` ownership and add bounded ATT transaction and attribute/service state. BLE SMP, long-term keys, privacy and bonding require a separate security model in a later slice.


### Stage 13.10J Bluetooth ATT/GATT authority and service model

ATT/GATT consumes `LeLinkState` as its lower-layer authority rather than treating an ATT attribute handle as global authority. `AttDatabase` is fixed-capacity, uses nonzero handles and bounded values, and rechecks the live LE controller handle on each transaction or discovery operation. Read/write permission checks remain in ATT so a higher GATT abstraction cannot bypass them.

`GattDatabase` maps Primary Service declarations and Characteristic Declaration/value pairs into the ATT attribute space. Characteristic declarations carry a value handle and bounded properties; characteristic values retain their own ATT permissions. Service and characteristic discovery are handle-range bounded and remain tied to the requesting live LE link.

`GattSubscriptions` records notification/indication authority as the tuple of LE connection handle and characteristic value handle. CCCD-style enable/disable state is bounded, invalid bit combinations are rejected, and every emitted Handle Value Notification or Indication rechecks both subscription mode and live LE ownership. Disconnect therefore makes retained subscription state unusable even before later cleanup/hardening.

This stage is a deterministic kernel protocol foundation, not a claim of complete physical BLE GATT interoperability. Stage 13.10K should build standard/custom BLE profile services on this authority boundary. BLE SMP, bonding/LTK state and persistent key protection remain separate security work and must not reuse BR/EDR link-key authority.


### Stage 13.10K BLE profile/service layer

The profile layer composes services from Stage 13.10J GATT primitives instead of bypassing ATT. Device Information and Battery Service attributes therefore inherit live `LeLinkState` checks and ATT read/write permissions. Battery level is bounded to 0..=100 and its notification path uses `GattSubscriptions` scoped to the exact LE handle and Battery Level value handle; state is committed only after notification authorization succeeds.

The WovenHat OS BLE service demonstrates the custom-service boundary with separate status and command characteristics. Status is readable but not writable; command is writable but not readable. A disconnected LE peer cannot continue using either path because ATT transactions recheck link ownership.

GATT characteristic creation preflights capacity for both declaration and value attributes before mutation. This preserves database consistency under fixed-capacity exhaustion and prevents partially-created characteristic state.

Stage 13.10L should harden controller/link/service lifecycle, teardown, malformed-event handling and recovery while retaining these ownership and permission boundaries.


### Stage 13.10L Bluetooth lifecycle authority

`BluetoothLeLifecycle` is the coordination boundary for LE link ownership and GATT subscription authority. Teardown is ordered: the live link is removed only after a structurally valid successful Disconnection Complete event identifies an owned handle; subscriptions for that exact handle are then revoked. Failed parsing or stale ownership returns an error before subscription or generation mutation.

Controller reset is a stronger invalidation boundary and clears all live LE links and all GATT subscriptions. The lifecycle generation advances on each successful disconnect or reset, providing a bounded epoch signal for later controller/service recovery work.

Reconnect deliberately creates no implicit subscription continuity. A re-established link must explicitly configure its GATT subscription again. This prevents stale CCCD-style authority from crossing disconnect/reset generations.

The Stage 13.10L stress contract repeats these invariants across 32 cycles and alternates disconnect and reset paths. It is a software-state hardening boundary, not evidence of physical radio/controller recovery or interoperability.


### Stage 13.10R Bluetooth secure-session identity authority

`LeSecuritySessions` stores the LE peer address type and address alongside the
controller handle. `session_for_link` is the security boundary used by both
ATT/GATT transactions and secured notification/indication emission: a session
is valid only while the live `LeLinkState` entry has the same handle and peer
identity. Handle reuse therefore cannot inherit the previous peer's protected
authority even if transient teardown cleanup was missed.

This remains bounded deterministic software state. It does not claim physical
BLE controller recovery, SMP/LTK provisioning, radio interoperability or
hardware qualification.


### Stage 13.10S BLE minimum key-size authority

`AttSecurityPolicy` carries an optional minimum encryption key size in addition
to its authentication requirement. `AttDatabase::transact_secured` and
`GattSubscriptions::emit_secured` both reject a live authenticated session
whose negotiated key is below that bound. The policy is checked after the
identity-bound session lookup and before either an ATT response or an outbound
notification/indication is produced, keeping inbound and outbound authority
consistent.


### Stage 13.10T BLE outbound authorization acceptance

The Stage 13.10 runtime gate requires a dedicated outbound key-size marker in
addition to the inbound ATT/GATT checks. Its self-test exercises the full
subscription path: weak authenticated sessions are denied before packet
encoding, and a strong identity-bound session produces the expected
notification.


### Stage 13.11A BLE SMP foundation

`BleSmpFixedChannel` is the bounded protocol boundary above a live
`LeLinkState` and L2CAP fixed CID `0x0006`. It parses and validates pairing
parameters before advancing `SmpPairingState`; malformed fields, unsupported
authentication/key-distribution bits, stale handles and non-SMP CIDs fail
closed. This layer does not claim cryptographic derivation, user interaction,
bond persistence or physical-controller qualification.


### Stage 13.11B BLE SMP LTK distribution

The SMP distribution boundary accepts legacy Encryption Information and Master
Identification only for the live negotiated pairing handle. It validates key
length, identity and negotiated-size consistency before storing material in
`LeBondStore`, preserving the existing address/type ownership model and
fail-closed teardown semantics.

Pending distribution retains the peer address identity captured with the
Encryption Information PDU. Master Identification is accepted only while the
same identity remains live, preventing handle reuse from importing stale LTK
material into another peer's bond.


### Stage 13.11C BLE SMP confirm/random verification

`SmpConfirmState` binds Pairing Confirm and Pairing Random processing to the
negotiated `SmpPairingState`, live handle and address-aware confirm inputs. The
`c1` and `s1` boundaries are explicit and bounded; a confirm mismatch enters
the failed state for that pairing attempt. This does not claim complete
production pairing UX or key-derivation integration.


### Stage 13.11F BLE controller encryption authority

`SmpConfirmState::start_encryption_command` owns the bounded LE Start
Encryption command layout and returns a pending authority record containing the
live peer identity, negotiated key size and authentication result.
`LeSecuritySessions::encryption_change_from_pairing` accepts only a matching
controller completion event and live link; failure, disabled state, stale
handles or identity mismatch revoke authority.


### Stage 13.11G BLE identity/privacy foundation

The bounded identity store accepts SMP Identity Information and Identity Address
Information only for the live pairing handle. IRK-backed private-address
resolution is deterministic and fail-closed, with duplicate identity replacement
remaining bounded. This identity layer complements, but does not replace, the
peer-bound LTK distribution and controller encryption authority checks.


### Stage 13.11H BLE SMP lifecycle and authentication closure

`SmpLifecycle` is the bounded orchestration boundary for legacy BLE SMP state.
It serializes Pairing Request/Response and key/identity distribution against one
live LE handle, rejects duplicate, out-of-order and wrong-handle traffic, and
clears transient distribution state on Pairing Failed, disconnect and reset.
Handle reuse therefore cannot silently inherit the previous pairing lifecycle.

Authentication authority is deliberately separated from association-model
negotiation. `SmpPairingState::authentication` selects Just Works, Passkey
Entry or OOB policy, but feature exchange alone does not mint persistent
authenticated authority. `SmpAuthenticationProof` supplies the method-specific
temporary key boundary: Just Works uses the zero TK and remains unauthenticated,
Passkey Entry accepts only 0..=999999 and encodes that value into the legacy TK,
and OOB requires an explicit 128-bit TK. Only
`SmpConfirmState::verify_random_with_proof` may promote a verified Passkey/OOB
attempt to authenticated session authority after Confirm/Random succeeds.

The compatibility raw-TK `verify_random` path may prove confirm consistency but
cannot mint MITM-authenticated authority. Likewise, legacy key distribution no
longer marks a persistent bond authenticated merely from negotiated pairing
capabilities. The Stage 13.11 runtime harness requires the G identity/privacy
marker, H lifecycle marker, and the persistent-authentication audit marker on
the 1/2/4-core acceptance matrix.

This closes the deterministic software authority model for Stage 13.11. It does
not claim Bluetooth qualification, physical-radio interoperability, side-channel
resistance, formal cryptographic verification, or an external security audit.
Those require separate hardware/specification validation before production
security claims.


## Stage 14.1 WovenNet IPv4 core closure (2026-10-06)

Stage 14.1 is the accepted IPv4 production-networking foundation above the
VirtIO-net/smoltcp runtime. The live QEMU/slirp gate exercises DHCP lease
acquisition, DNS A-record resolution, ICMP reachability, host-to-guest UDP
traffic and a bidirectional TCP round trip on the same runtime used by normal
boots. The release workflow runs this contract on the 1/2/4-core matrix with
the `stage14-1-test` feature and requires the aggregate
`[S14.1] WovenNet IPv4 core: PASSED` marker.

DHCP transition authority is centralized in `apply_dhcp_locked` and
`apply_static_locked`. A resolver change cancels every outstanding bounded
DNS query before the DNS server is replaced, so a response issued under an old
resolver cannot be mistaken for work belonging to the new lease. The
`[S14.1D]` acceptance test drives DHCP-to-lease and lease-to-static recovery
and verifies address, prefix, gateway, resolver and query cleanup.

DNS query IDs are generation-tagged capabilities rather than reusable raw slot
numbers. `dns_start` records a non-zero generation with the smoltcp query
handle; `dns_poll` and `dns_cancel` require the same slot/generation pair.
After cancellation or slot reuse, an old token therefore fails with
`SocketError::Invalid` and cannot poll or cancel the replacement query. This
is covered by the `[S14.1G]` and `[S14.1H]` lifecycle gates.

Userspace sockets remain bounded to `MAX_USER_SOCKETS = 16`. Ownership is
checked on descriptor operations, while asynchronous operations use
`SocketToken { slot, generation, owner }` so stale pinned authority cannot
cross descriptor reuse. Close revokes descriptor access immediately; UDP can
retire immediately when unpinned, while TCP retains transport state only for a
bounded graceful-drain interval. The live network test now explicitly retires
its TCP owner and waits for `user_sockets == 0` before the resource-stress
gate begins, preventing acceptance-test resources from leaking into the next
authority domain.

The `[S14.1S]` stress gate fills all 16 userspace socket slots, requires the
17th open to return `SocketError::NoSlot`, retires the full set, fills the
table again with unique in-range descriptors, verifies exhaustion again, and
requires the table to return to zero. It also verifies dynamic/private
ephemeral allocator wraparound across `65533 -> 65534 -> 49152 -> 49153`.
Failure checkpoints remain in the test so future regressions identify the
broken phase rather than collapsing into a generic marker failure.

Stage 14.1 closure was accepted by GitHub Actions release-validation run #1199
(run 37337619836) at commit
`c1cce4da89ab7527290b1616207139a1b038b05a`. The full release-validation job
was green, including the live Stage 14.1 1/2/4-core acceptance matrix and all
earlier mandatory release gates.

### Stage 14.1 boundary and follow-up hardening

This closure establishes the bounded IPv4 core and its current lifecycle
invariants; it is not a claim of a complete POSIX/BSD socket stack, IPv6,
production firewall/NAT policy, TLS, physical-NIC interoperability across
hardware families, or exhaustive network fault injection. The current
ephemeral-port stress proves range/wrap behavior and bounded socket recovery;
it does not yet exhaustively prove collision avoidance across every concurrent
TCP/UDP local-endpoint combination. Future networking work should add explicit
live-endpoint collision selection/retry semantics before making that stronger
claim.

### Stage 14.2 IPv6 closure

Stage 14.2 is accepted for the bounded WovenNet IPv6 scope defined by the
master roadmap: IPv6, DHCPv6, and IPv6 Neighbor Discovery.

The implementation includes bounded IPv6 address/prefix types and
solicited-node multicast derivation; ICMPv6 Neighbor Discovery parsing for
RS/RA/NS/NA; IPv6 pseudo-header checksum generation/verification; router and
prefix lifetime state; neighbor reachability and Duplicate Address Detection;
SLAAC address lifecycle; DHCPv6 message parsing/serialization, client
Solicit/Advertise/Request/Reply state, and IA_NA/IAADDR lease lifetimes; an
interface-level ICMPv6 ingress state boundary; and runtime installation of a
MAC-derived IPv6 link-local /64 alongside the existing IPv4 address.

The closure hardening gate rejects NDP traffic whose IPv6 Hop Limit is not
255. It also requires link-local Router Advertisement sources, rejects
Neighbor Advertisements from the unspecified source, and constrains DAD
Neighbor Solicitations from the unspecified source to the tentative address's
solicited-node multicast destination. Existing ICMPv6 checksum validation
remains mandatory before state mutation.

The accepted serial closure markers include
`[S14.2J] WovenNet IPv6 interface ingress: PASSED`,
`[S14.2K] WovenNet runtime IPv6 integration: PASSED`, and
`[S14.2L] WovenNet NDP ingress hardening: PASSED`. The network QEMU harness
carries these requirements forward into the Stage 14.3 and Stage 14.4 feature
gates so later networking work cannot silently regress the accepted IPv6
boundary.

This acceptance is intentionally scoped. It does not claim physical-NIC IPv6
interoperability across hardware families, a complete POSIX/BSD IPv6 socket
surface, production route ownership/policy, or exhaustive RFC 4861/8415
interoperability. Those concerns remain appropriate follow-up hardening or
belong to Stage 14.3+.

### Stage 14.3 socket API closure

Stage 14.3 is accepted for the bounded WovenNet Socket API scope.

The existing owner-scoped socket table remains the authority root for UDP and
TCP open, bind/listen, connect, send, receive, peer inspection, close, and
process cleanup. Descriptors are checked against their owning process, while
asynchronous work uses `SocketToken` values containing the slot, owner, and
generation. Closing a descriptor revokes descriptor authority immediately;
after outstanding pins are released and the slot is reused, an old token
cannot operate on the new socket generation.

The endpoint boundary preserves the legacy packed IPv4 representation for
compatibility and adds `SocketEndpointV1`, a fixed-layout versioned endpoint
representation for IPv4 and IPv6. It rejects zero ports, malformed IPv4
encodings, unspecified or multicast destinations, unknown address families,
and non-zero reserved ABI fields. The versioned representation is wired into
real socket connect and peer inspection operations rather than existing only
as a serialization helper.

The accepted serial gates are
`[S14.3] WovenNet socket API boundary: PASSED` and
`[S14.3A] WovenNet socket authority lifecycle: PASSED`. The QEMU networking
harness requires both markers for Stage 14.3 and carries them forward into
Stage 14.4, preventing routing work from silently weakening socket authority.

This acceptance does not claim a complete POSIX/BSD socket surface,
exhaustive physical-NIC IPv6 interoperability, or production routing-policy
ownership. Those remain later platform hardening or Stage 14.4+ concerns.

### Stage 14.4 routing-table closure

Stage 14.4 is accepted for the bounded WovenNet routing-policy scope.

`WovenRouteTable` is a fixed eight-slot, generation-tagged, owner-scoped
dual-stack policy table. IPv4 prefixes from /0 through /32 and IPv6 prefixes
from /0 through /128 are normalized before storage. Route lookup rejects
address-family mismatches and selects the longest matching prefix, then the
lowest metric and oldest generation as deterministic tie-breakers. Invalid
prefixes, unusable gateways, and mixed network/gateway address families are
rejected at insertion.

Each `WovenRoute` records its owner and generation. Exact route handles are
required for removal, cross-owner removal returns `WrongOwner`, and
`remove_owner_routes` provides bounded deterministic teardown when an
authority domain exits. This gives later WovenGuard and Network Manager work
an explicit routing authority boundary instead of a globally mutable table.

The policy table is now connected to the real smoltcp route set through
controlled `install_woven_route` and `remove_woven_route` boundaries.
Installation is idempotent for an identical CIDR/gateway pair and reports
bounded-capacity failure rather than silently dropping a route. Removal is
exact and a stale second removal is rejected. The live acceptance gate proves
this lifecycle for both an IPv4 route and an IPv6 /64 route without replacing
the existing DHCP/static default route.

The accepted serial gates are
`[S14.4] WovenNet routing table: PASSED` and
`[S14.4A] WovenNet live route integration: PASSED`. The Stage 14.4 QEMU
harness requires the live marker in addition to the inherited Stage 14.2 and
14.3 networking gates.

This acceptance does not claim a userspace route-management service, dynamic
routing protocols, multi-interface policy routing, or exhaustive physical-NIC
interoperability. Those remain appropriate Network Manager and platform
hardening work. Stage 14.5 can now build WovenGuard firewall policy on the
accepted socket, IPv6, and routing authority foundations.

### Stage 14.5A WovenGuard firewall policy foundation closure

Stage 14.5A is accepted for the bounded policy foundation scope.

`FirewallPolicy` stores at most 32 rules and fails closed on capacity,
duplicate identifiers, invalid IPv4/IPv6 prefix lengths, and reversed port
ranges. Rules match inbound, outbound, or forward traffic by protocol,
optional source/destination CIDR, and optional source/destination port range.
Evaluation is deterministic: the lowest numeric priority wins and the rule ID
is the stable tie-breaker. Traffic with no matching rule receives the
configured default action.

IPv4 and IPv6 prefix matching covers /0 through /32 and /0 through /128,
including non-octet prefix lengths. Global policy access is serialized through
the IRQ-aware mutex boundary so later live enforcement does not introduce an
unsynchronized mutable policy table.

The accepted serial gate is
`[S14.5A] firewall policy foundation PASSED`. The Stage 14.5 feature inherits
the accepted Stage 14.1-14.4 networking gates and is exercised by the QEMU
network harness on 1, 2, and 4 CPUs. Acceptance was recorded by GitHub Actions
run #1267 on commit `f8b79ac7`.

This closure deliberately does not claim live filtering. Stage 14.5B must
connect policy evaluation to real ingress and egress packet paths while
preserving DHCP, NDP/RA/SLAAC, socket authority, and routing behavior. A
forwarding hook should be added only where an actual forwarding path exists.


### Stage 14.5B WovenGuard live packet enforcement closure

Stage 14.5B is accepted for the live virtio ingress/egress enforcement scope.
The firewall now has a packet-path bridge from Ethernet frames to the Stage
14.5A policy model. IPv4 and IPv6 headers are decoded into source/destination
addresses and TCP/UDP ports or ICMP/ICMPv6 protocol identity before policy
evaluation.

The enforcement points match the smoltcp device contract. Inbound denied
frames are discarded in `VirtioSmolDevice::receive` before an RX token is
exposed to smoltcp. Outbound frames are evaluated in `WovenTxToken::consume`
after smoltcp has constructed the Ethernet frame and before
`virtio_net::transmit` submits it. This avoids pretending that an
`RxToken::consume` callback can cancel delivery after a token has already
been returned.

Non-IP Layer-2 frames are preserved at this stage so ARP and other link-control
traffic are not accidentally removed by an IP firewall parser. IPv6
ICMPv6 policy metadata supports the NDP/RA/SLAAC control plane inherited from
Stage 14.2.

Live enforcement uses an explicit atomic activation state. The policy itself
retains default-deny semantics, but enforcement remains inactive during
bootstrap and becomes active only through the firewall lifecycle boundary.
This separation was required after the first Stage 14.5B gate correctly
exposed a DHCP bootstrap deadlock when default-deny enforcement was active
from boot.

Acceptance requires `[S14.5B] live packet enforcement PASSED` plus all
inherited WovenNet markers. GitHub Actions run #1277 on commit `1d1ce646`
passed build, Clippy with `-D warnings`, live DHCP/DNS/ICMP/UDP/TCP
regressions, and the dedicated Stage 14.5B QEMU matrix on 1, 2, and 4 CPUs.

This closure does not claim a userspace firewall administration API, persistent
rule configuration, logging/counters, stateful connection tracking, NAT, an
actual routed forwarding hook, or completed Wi-Fi enforcement parity. Those
remain later WovenGuard/network-management work.


### Stage 14.5C WovenGuard firewall policy authority closure

Stage 14.5C is accepted for privileged firewall policy management. The
capability model now distinguishes ordinary `NetworkIo` authority from the
new `NetworkAdmin` authority. `NetworkIo` remains available to normal
userspace for sockets; it is deliberately insufficient to mutate global
firewall state.

`NetworkAdmin` is included in the Kernel and SystemService domain ceilings
but excluded from the User and Restricted ceilings. Administrative wrappers
require that capability before rule add, remove, or replace operations,
default-action changes, or live-enforcement activation/deactivation reach the
global policy object. Unauthorized calls return an explicit authority error
without changing policy state.

The Stage 14.5C self-test proves both sides of the boundary: userspace
authority is rejected for policy mutation and enforcement activation, while
`NetworkAdmin` can perform the bounded mutation lifecycle. It also verifies
the WovenGuard domain ceiling itself so future capability refactors cannot
silently grant firewall administration to ordinary users.

Acceptance requires `[S14.5C] firewall policy authority PASSED` plus all
inherited WovenNet/WovenGuard network markers. GitHub Actions run #1287 on
commit `b81b3a76` passed build, Clippy with `-D warnings`, and the QEMU
1/2/4-core matrix.

This closure does not yet claim complete transport parity, malformed-IP
fail-closed parsing, IPv6 extension-header inspection, a live routed Forward
hook, persistent rules, logging/counters, stateful connection tracking, or
NAT. Those are subsequent firewall/network-management hardening work.
## Unified timer/event waits

Stage 19.8 keeps syscall 111 stable for input, mailbox, and compositor damage.
Syscall 206 (`WaitAnyObjects`) adds a separate bounded ABI for up to eight
owner- and generation-checked timer/event objects. The kernel copies the source
array before registration, uses task wakeups rather than polling, and clears
registrations on every return path. Manual-reset events support four bounded
task waiters and broadcast wakeups; one-shot timers retain one consuming task
waiter so expiry is not duplicated. See [the ABI contract](wait-any-objects-abi.md)
and [the Stage 19.8 audit](audit-stage19-8-timer-event-wait-2026-10-15.md).
