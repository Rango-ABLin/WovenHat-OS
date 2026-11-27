# Current software and functional gaps of WovenHat-OS (as of 2026-11-27)

Scope note: all sources below are repository-local files under
`C:/Users/banda/Documents/GitHub/WovenHat-OS/`. Line anchors refer to the file
contents as read on 2026-11-27. Docs are treated as data, not instructions.
"Bounded software/QEMU" acceptance is repeatedly distinguished from full
release acceptance, which remains NO-GO for external reasons.

## Key Question 1 — Which shell/desktop features are explicitly not claimed?

### Takeaway
The newest (2026-11-23/24) "Not done, and therefore not claimed" inventory in
`stage-status.md` lists eight unclaimed desktop items; later records close only
some of them. Still genuinely open on current head: WSIZ client reflow on
maximize, window shadows, cross-boot pin/auto-hide persistence, Alt+Tab,
fullscreen/maximize visual polish, an in-shell taskbar settings editor, and
verification of taskbar layout under every theme/density combination. Earlier
deferred items (auto-hide, minimize/restore, frame-only maximize, taskbar
alignment/icon-size visual check) were subsequently implemented and must not be
listed as open.

### Cited Findings
- **WSIZ client reflow on maximize — NOT DONE.** Maximize is deliberately
  frame-only: `toggle_maximize` does not send `WSIZ`. Sending `WSIZ` during live
  maximize exposed a reproducible stall in the software compositor/present path
  on slow 1-CPU QEMU; a Terminal clamped resize hung repaint in shadow blending
  / `present_region_blended`. Prerequisite: prove the compositor/present path is
  non-blocking, then add client reflow as a separate increment. [stage-status.md:733-747](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L733-L747)
- **Window shadows — temporarily disabled.** For the same stall reason, window
  shadows are disabled in `repaint_once` with an explicit TODO; the doc calls it
  "a visual regression recorded here, not a hidden behavior change."
  Prerequisite: same non-blocking compositor proof. (Older accepted Stage 19.13
  translucency/shadow and M01 Stage 20.5 focused-shadow gates do not restore the
  current shell's window shadows.) [stage-status.md:738-747](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L738-L747); [m00-baseline-ledger.md:53-57](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/m00-baseline-ledger.md#L53-L57)
- **Cross-boot pin and auto-hide persistence — NOT DONE.** Pins and auto-hide
  are seeded from the installed settings record but are in-session state only:
  the shell holds `FileRead` and has no settings-write path, so nothing claims
  they survive a reboot. Prerequisite: a writable settings record plus a
  deliberate `FileWrite` capability decision for the shell (or a service proxy).
  Repeated at three dates, latest 2026-11-25. [stage-status.md:624-630](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L624-L630); [stage-status.md:667-678](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L667-L678); [stage-status.md:748-750](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L748-L750); [agent-coordination.md:497-501](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/agent-coordination.md#L497-L501)
- **Alt+Tab switcher — BLOCKED.** `libwoven::Key` has no modifier-key concept,
  so Alt+Tab is indistinguishable from bare Tab at the ABI level. Bare Tab
  already gives MRU cycling via `raise()`, but that is not a labeled Alt+Tab
  switcher. Prerequisite: an input-ABI change adding modifier state — described
  as a new ABI stage touching kernel files that carried another agent's
  uncommitted edits (2026-11-23). [stage-status.md:751-753](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L751-L753); [overview-desktop-2026-11-03.md:271-283](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/graphics/overview-desktop-2026-11-03.md#L271-L283); [agent-coordination.md:484-486](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/agent-coordination.md#L484-L486)
- **Fullscreen/maximize visual polish — NOT DONE.** Explicitly in the unclaimed
  list; no closure found in newer records. [stage-status.md:751](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L751)
- **In-shell taskbar settings editor — NOT DONE.** Only Quick Settings toggles
  exist; a dedicated editor is "still not claimed." Prerequisite: `FileWrite`
  capability or service proxy (same as pin persistence). [stage-status.md:753](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L753); [stage-status.md:803-805](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L803-L805)
- **Taskbar layout under every theme/density combination — NOT VERIFIED.** Only
  the `left`+`large` case got a QEMU pixel check; other combinations remain
  "not claimed." [stage-status.md:805-807](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L805-L807)
- **Quick Settings Battery saver / Night light — SUPERSEDED, not an open gap.**
  The 2026-11-23 record says they were local toggles with no effect; on
  2026-11-24 they were replaced by `Taskbar align` and `Icon size` in-session
  actions. Battery status is still honestly "Unavailable" because there is no
  ACPI battery driver. [stage-status.md:753-755](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L753-L755); [stage-status.md:789-794](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L789-L794); [stage-status.md:1128-1129](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L1128-L1129)
- **Closed since the 2026-11-23 deferral — do not report as open:** taskbar
  auto-hide (implemented `d511ded4`, accepted 1/2/4 CPUs); minimize/restore
  (`134e8cbd`); frame-only maximize/restore (2026-11-23/24); taskbar alignment
  and icon-size CLI/QS controls and a left+large QEMU pixel gate
  (`test-taskbar-layout.py`, 2026-11-24). [stage-status.md:667-743](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L667-L743); [stage-status.md:758-787](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L758-L787)
- Older 2026-11-23 statement "minimize/restore, maximize/fullscreen and Alt+Tab
  deferred" is partially superseded by the frame-only maximize/minimize work;
  only fullscreen and Alt+Tab remain. [stage-status.md:1096-1105](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L1096-L1105)
- Phase B §24 acceptance is explicitly not claimed. [stage-status.md:807](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L807)

### Inferences
- The common root cause of the two biggest visual gaps (reflow and shadows) is
  one unresolved compositor/present non-blocking property on slow 1-CPU QEMU,
  not two independent feature decisions.
- Persistence gaps and the settings-editor gap share one prerequisite: giving
  `woven-shell` write authority (or a service proxy) has been deliberately
  declined, so these are policy-gated rather than merely un-coded.
- Alt+Tab is ABI-gated (`Key` has no modifier concept), not just unimplemented
  shell code.

### Gaps
- No document found that states the compositor/present stall has been diagnosed
  or fixed; no target date for the non-blocking proof.
- No record found of a deliberate `FileWrite`-for-shell decision being made or
  rejected with finality beyond "deferred."

## Key Question 2 — Which kernel/runtime limitations remain?

### Takeaway
Current-head open items are the absent shared cross-process font mapping ABI,
the compositor/present stall, and the pointer-step flake. Several items that
older entries list as open (font `busy` flag, `mem::transmute` `'static`, missing
large-map self-test, `keyboard_refresh` flake) were explicitly CLOSED by newer
2026-11-23/26 records and must not be reported as current gaps.

### Cited Findings
- **Per-process font copies; no shared cross-process read-only font mapping ABI
  with a written threat review — STILL OPEN.** Newest wording: "Per-process
  mappings remain the accepted design." Repeated three times on 2026-11-26.
  Prerequisite: a shared cross-process read-only mapping ABI plus threat review.
  [stage-status.md:107-108](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L107-L108); [stage-status.md:134-136](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L134-L136); [stage-status.md:230-233](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L230-L233)
- **Compositor/present stall — STILL OPEN.** Reproducible hang in shadow
  blending / `present_region_blended` under a live-maximize `WSIZ` on slow
  1-CPU QEMU (see Key Question 1). [stage-status.md:733-738](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L733-L738)
- **Pointer-step flake — STILL OPEN.** Intermittent "mouse click did not change
  the client framebuffer"; explicitly "NOT claimed fixed" by the 2026-11-23
  `keyboard_refresh` analysis and still listed open in the newest Phase B
  record. Prerequisite: diagnose harness vs guest; a closable virtual model is
  absolute-coordinate injection (an xHCI `usb-tablet` `--absolute-click` model
  already exists for WovenFiles, but not for the editor relative-click mode).
  [stage-status.md:607-609](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L607-L609); [stage-status.md:755-756](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L755-L756); [stage-status.md:1177-1180](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L1177-L1180); [stage-status.md:380-384](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L380-L384)
- **CLOSED (superseded) — font face cache transient `busy` flag and
  `mem::transmute` to extend a lifetime.** The 2026-11-26 "Font face cache
  hardening" entry says `load_system_font_mapping` now returns `&'static [u8]`
  (no transmute) and the face cache waits on a bounded spin-lock instead of
  returning `None` on transient contention. These two were still listed open in
  the same-day "address-layout collision" entry; prefer the newer/closer-to-top
  closure record. [stage-status.md:110-117](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L110-L117); contradicted by [stage-status.md:230-233](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L230-L233)
- **CLOSED (superseded) — absent focused large-mapping kernel self-test.**
  Restored 2026-11-26 behind its own `large-map-test` feature with 1 GiB guest
  RAM (the original was removed because it caused TCG frame pressure in the
  256 MiB stress matrix). [stage-status.md:89-105](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L89-L105)
- **CLOSED (superseded) — `keyboard_refresh` flake.** Root-caused 2026-11-23 as
  a harness oracle defect (hard-coded digit-2 bitmap); oracle replaced, no
  failure since. Separate from the pointer-step flake. [stage-status.md:580-609](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L580-L609)
- **Guest-terminated TLS — BLOCKED (one prerequisite remains).** Secure RNG is
  no longer missing: syscall 251 (`SecureRandom`) was added 2026-11-25 with a
  bounded HMAC-SHA256 DRBG, but TCG boots honestly report
  `[RNG] boot entropy: unavailable (fails closed)` because QEMU's default CPU
  has no health-checked hardware source; it is a virtual/software entropy ABI,
  not a certified TRNG. Remaining blocker for Ring-3-terminated TLS: no
  `no_std` TLS provider builds for `x86_64-unknown-none` here (`ring` has no C
  sysroot); a feasibility spike shows `rustls` itself compiles without `ring`,
  so the blocker is implementation effort for a custom `CryptoProvider`, not a
  toolchain wall. TLS remains terminated by the host mTLS proxy. Note the
  older listen/accept entry's claim that "no production secure-RNG syscall"
  exists is superseded by the later secure-RNG entry. [stage-status.md:261-287](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L261-L287); [stage-status.md:289-320](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L289-L320); [agent-coordination.md:89-103](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/agent-coordination.md#L89-L103)
- **Listener is deliberately single-connection.** smoltcp 0.14 has no public
  `accept()`/backlog, so no backlog is fabricated; a second client needs a new
  socket. Accepted limit, not a defect. [stage-status.md:294-297](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L294-L297)
- **Bounded-scope exclusions (S00–S20 inventory, dated 2026-10-24, content
  updated later):** full POSIX compatibility and physical HID qualification are
  outside S03; libc/pthread/locale/math/destructors/TLS/shared libraries are
  outside the freestanding C ABI S02; IPv6 socket lifecycle, routes/ND, profile
  service and full TLS certificate lifecycle remain incomplete (S09); generic
  driver crash supervision, hotplug breadth and power transitions are open
  (S08); disk-full logging and privilege-separated service qualification are
  open (S07). [software-gap-inventory.md:19-30](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/software-gap-inventory.md#L19-L30)
- **Known stale architecture-guide claims:** "Persistent account provisioning
  remains open" is superseded by persistent account import implemented
  2026-10-20; "Settings are not persisted" is superseded by persisted theme and
  reduced motion on 2026-11-27. [architecture-and-codebase-guide.md:1295](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/architecture-and-codebase-guide.md#L1295); [stage-status.md:8420-8438](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L8420-L8438); [stage-status.md:3-16](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L3-L16)

### Inferences
- The font limitation is a design choice (per-process copies accepted), not a
  bug; closing it requires both an ABI and a threat review, so it is a stage,
  not a patch.
- The "no secure-RNG syscall" blocker for guest TLS has been removed; the
  remaining guest-TLS work is a custom pure-Rust `CryptoProvider` integration.
- Several widely-cited "flakes" have been retired; the pointer-step flake is
  the only input flake still explicitly open.

### Gaps
- No evidence found that the compositor/present stall has been root-caused
  beyond the observed `present_region_blended` hang.
- No reliable current status found for "runtime multi-thread QEMU qualification"
  (listed open at stage-status.md:2522) or the compiler-fixture gate
  (architecture-and-codebase-guide.md:3335) after later runtime-TLS work.

## Key Question 3 — Which application-level gaps remain?

### Takeaway
The 2026-11-25 app integration closed bounded Open/Save, settings duplicate-key
handling and mutation CLI work, but explicitly left full editor GUI pointer
behavior and live theme propagation open — and live theme/reduced-motion
propagation was then closed on 2026-11-27. Remaining app-level gaps: full editor
GUI pointer behavior, the editor overwrite-tail ABI limit, the `/mnt`
provisioning gap in the disposable FAT32 harness, and guest TLS termination.

### Cited Findings
- **WovenEditor GUI — bounded controls only; full GUI pointer behavior NOT
  DONE.** 2026-11-25 added bounded Open, Save, Save As, path field, write+sync
  marker checks and an unsaved-close hold. But "Full editor GUI behavior under
  pointer input remains open," and the editor relative-click desktop mode fails
  identically on the pre-change baseline, so it is recorded as a pre-existing
  harness limitation, not editor GUI acceptance. Prerequisite: absolute-coordinate
  injection (or a WAX1/`DesktopServiceBroker` action path) plus a
  provider-policy test. [APPLICATION-INTEGRATION-REPORT.md:23-34](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/APPLICATION-INTEGRATION-REPORT.md#L23-L34); [APPLICATION-INTEGRATION-REPORT.md:53-59](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/APPLICATION-INTEGRATION-REPORT.md#L53-L59); [stage-status.md:256-259](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L256-L259)
- **Editor overwrite tail — accepted ABI limit.** There is no truncate/rename
  primitive in the established ABI, so overwriting a longer note with shorter
  text retains a tail; Save As to a fresh path is clean. Documented as accepted
  "rather than papered over with a new syscall." Prerequisite to close: a new
  truncate ABI primitive (deliberately not added). [APPLICATION-INTEGRATION-REPORT.md:30-34](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/APPLICATION-INTEGRATION-REPORT.md#L30-L34)
- **WovenSettings live propagation — CLOSED on 2026-11-27 (do not report as
  open).** 2026-11-15/25 records say light/dark remain session previews and live
  propagation to an already-running shell is a separate increment; the newest
  entry says WovenShell's periodic settings observer applies theme and
  reduced-motion changes live without write authority. Persistence of
  fixed-width Dark/Light/High Contrast and reduced motion is accepted, with
  fail-closed malformed values. [APPLICATION-INTEGRATION-REPORT.md:35-40](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/APPLICATION-INTEGRATION-REPORT.md#L35-L40); [stage-status.md:258-259](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L258-L259); [stage-status.md:3-16](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L3-L16)
- **WovenFiles `/mnt` provisioning gap — OPEN harness limitation.** The mutation
  CLI (`--mkdir`, `--rm`, `--rename`, `--write`) is confined to `/mnt` or `/tmp`
  and passes at 1/2/4 CPUs, but the sequence runs under `/tmp` because a
  disposable FAT32 image without a provisioned `/mnt` directory cannot satisfy
  the kernel's `ensure_path("/mnt")` precondition. "That gap is documented here,
  not hidden." Prerequisite: provision `/mnt` in the disposable FAT32 image to
  exercise the native-storage path end to end. [APPLICATION-INTEGRATION-REPORT.md:41-51](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/APPLICATION-INTEGRATION-REPORT.md#L41-L51); [stage-status.md:249-256](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L249-L256)
- **Remote admin TLS — host proxy remains the TLS boundary.** Guest-side
  listener and secure RNG now exist (see Key Question 2), but Ring-3-terminated
  mTLS is still blocked on a custom `no_std` TLS provider. Prerequisite: a
  pure-Rust `CryptoProvider` for `rustls` on `x86_64-unknown-none`; physical
  entropy review is a separate release gate. [stage-status.md:316-320](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L316-L320); [stage-status.md:282-287](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L282-L287)
- **Certified-printer / IPP TLS — partially closed in virtual model, physical
  remains open.** A disposable CA + valid IPP response server now passes, and it
  exposed/fixed a real `libwoven::ipp_tls::content_length` defect; certified
  printer, issuing CA DER and native microphone remain strictly external.
  [stage-status.md:385-396](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L385-L396); [software-gap-inventory.md:30](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/software-gap-inventory.md#L30)
- **WovenImages decoding — CLOSED (superseded).** The 2026-10-19 guide says
  "Image decoding and input-driven selection are not implemented," but the
  2026-10-25 Stage 07 entry says WovenImages now loads and decodes the VFS asset
  and draws its thumbnail, and a bounded pointer Refresh control was accepted
  2026-10-19. Do not report image decoding as an open gap.
  [architecture-and-codebase-guide.md:1597-1604](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/architecture-and-codebase-guide.md#L1597-L1604); [stage-status.md:3935-3945](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L3935-L3945); [stage-status.md:8522-8524](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L8522-L8524)
- **Broader service/app persistence — specific gaps remain:** persistent policy
  service integration (stage-status.md:1963), "broader application persistence"
  (stage-status.md:1599), and fresh-store/service IPC persistence
  (stage-status.md:2090-2125) are listed open or partially open; these are
  lower-priority than the ranked items above and dated earlier.
- **Account creation — still open** per the 2026-11-era accounts note
  (stage-status.md:6735), though persistent account import exists
  (stage-status.md:8420-8438).

### Inferences
- App gaps cluster into three classes: harness-input limitations (editor
  relative-click, `/mnt` provisioning), a deliberate ABI limit (truncate), and
  an unimplemented crypto provider (guest TLS).
- Live settings propagation is a recent closure that invalidates the
  2026-11-15/25 reports; using those alone would overstate the gap.

### Gaps
- No document gives a firm closure plan or owner for full editor GUI pointer
  acceptance beyond naming the absolute-coordinate model.
- No current status found for whether `/mnt` has since been provisioned in the
  disposable image; latest word is "documented, not hidden."

## Key Question 4 — Which failing/flaky gates are preserved as evidence rather than fixed?

### Takeaway
The repository deliberately preserves failed runs as evidence, reruns unchanged
harnesses, and does not widen budgets. No current failure is known to be an
unfixed regression; the one input flake still open is the pointer-step issue.

### Cited Findings
- **Pointer-step flake preserved, not fixed:** "mouse click did not change the
  client framebuffer" caused two dedicated 1-CPU reruns to fail; it cleared on
  the second run but is explicitly still open. [stage-status.md:1177-1180](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L1177-L1180); [stage-status.md:607-609](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L607-L609)
- **2-CPU desktop qualification timeout** on marker `WovenFiles: pointer
  cancelled` during the maximize work; same-tree rerun passed, no budget or
  assertion weakened. [stage-status.md:743](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L743)
- **1-CPU startup budget miss** (121,226 ms vs 120,000 ms) attributed to host
  load from the preceding gate's teardown; identical tree rebooted in 90,406 ms
  and passed. [stage-status.md:659-665](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L659-L665)
- **Four taskbar-pins gate-side defects preserved** (mismatched cursor frames,
  stale-variable crash after guest success, fixed-sleep vs repaint race,
  off-screen comparison box); the guest produced every expected marker/pixel.
  [stage-status.md:705-717](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L705-L717)
- **1-CPU taskbar-layout failure** `[SESSION] desktop not started: Image`
  because `MAX_ARGV=8` was too small; fixed by the 8→16 argv bump, evidence
  preserved. [stage-status.md:783-787](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L783-L787)
- **Stale wallpaper/text oracles** caused Stage 20.1/20.3/20.9 to fail on the
  pre-fix commit; recorded as pre-existing, not regressions. Stage 20.1 was
  requalified green on 2026-11-26; no explicit closure found for 20.3/20.9.
  [stage-status.md:169-174](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L169-L174); [stage-status.md:38-50](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L38-L50)
- **Release-qualification preserved failures:** restricted-network chain failed
  at DNS; a Stage 10.5 packaging run failed with Windows `StorageFull` (error
  112) writing the FAT image; a post-repair run failed at error 112 and a
  one-CPU retry passed; pre-repair/ASLR/compiler/TLS fixture build failures are
  retained; no assertion, warning policy or timeout was weakened.
  [TEST-QUALIFICATION-REPORT.md:83-85](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/TEST-QUALIFICATION-REPORT.md#L83-L85); [TEST-QUALIFICATION-REPORT.md:149-168](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/TEST-QUALIFICATION-REPORT.md#L149-L168); [BUILD-AND-BOOT-REPORT.md:64-75](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/BUILD-AND-BOOT-REPORT.md#L64-L75)
- **Editor relative-click desktop mode** fails identically on the pre-change
  baseline; recorded as pre-existing harness limitation, not editor GUI
  acceptance. [APPLICATION-INTEGRATION-REPORT.md:55-59](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/APPLICATION-INTEGRATION-REPORT.md#L55-L59)
- **Global `cargo fmt` check** still reports unrelated pre-existing kernel
  formatting drift in at least one 2026-11-26 record.
  [agent-coordination.md:44](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/agent-coordination.md#L44)

### Inferences
- The project's convention is to preserve failure artifacts, rerun on a quiet
  host, and refuse to weaken assertions — so "preserved failure" generally does
  not mean an open product defect.
- The pointer-step flake is the exception: it is preserved *and* still open.

### Gaps
- No single index maps every preserved failure artifact to a final disposition;
  dispositions are scattered across stage-status and release-qualification.

## Key Question 5 — Scope, dates, and superseded status (bounded vs full acceptance)

### Takeaway
Almost every "VERIFIED-SOFTWARE" classification is explicitly bounded to
software/QEMU and not a release claim. The S00 inventory header (2026-10-24) is
stale but its table content was updated through 2026-11-26; several of its rows
and the m00 ledger were corrected by later records. Final release status is
NO-GO for external reasons, not because of the software gaps inventoried here.

### Cited Findings
- "Verified-software" explicitly does not mean native hardware support,
  foreign-OS compatibility, production trust or release approval; source
  baseline 2026-10-24, commit `f510ed5`, but the table includes 2026-11-26
  events, so the header date is stale relative to content.
  [software-gap-inventory.md:1-11](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/software-gap-inventory.md#L1-L11)
- The inventory's S20 row is `BLOCKED` ("current release checkpoint remains
  NO-GO"); S10, S13, S15, S17, S19 are PARTIAL with software slices closed but
  sequential prerequisites and external custody/review still open.
  [software-gap-inventory.md:27-37](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/software-gap-inventory.md#L27-L37)
- m00 ledger (dated 2026-10-24, baseline `3eee80b8`) records M15 as NO-GO with
  17 native categories `PHYSICAL_NOT_TESTED`; its "intermittent desktop journey
  unaccepted" row is stale given later bounded desktop qualification passes.
  [m00-baseline-ledger.md:1-20](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/m00-baseline-ledger.md#L1-L20); [m00-baseline-ledger.md:161-171](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/m00-baseline-ledger.md#L161-L171)
- Agent coordination explicitly corrected stale S-slice records: the
  OVMF/swtpm/signed-guest virtual trust chain is already QUALIFIED in WSL2, and
  S15/S17/S19 software paths are local-pass; remaining blockers are external
  custody/review/native items plus small virtual models — later implemented as
  disposable fail-closed models on 2026-11-25.
  [agent-coordination.md:282-300](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/agent-coordination.md#L282-L300); [stage-status.md:369-412](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L369-L412)
- External gates repeated in the newest 2026-11-27 entry: native input/GPU
  performance and device-loss qualification; production non-exportable keys,
  witnessed custody, signed exact-commit artifacts, independent build/review
  attestation, physical Secure Boot/TPM evidence.
  [stage-status.md:18-21](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/stage-status.md#L18-L21)
- `REMAINING-WORK.md` (2026-11-15) still ranks "reconcile the next dependency-
  ordered Stage 11 executable-layout gap" as open; this conflicts with the
  later loader-preservation increment accepted via 54 checks/36 QEMU boots.
  [REMAINING-WORK.md:7-23](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/REMAINING-WORK.md#L7-L23); [TEST-QUALIFICATION-REPORT.md:138-168](C:/Users/banda/Documents/GitHub/WovenHat-OS/docs/release-qualification/TEST-QUALIFICATION-REPORT.md#L138-L168)

### Inferences
- For a report, treat "software gap" and "release blocker" as separate axes:
  most release blockers are external and are not unimplemented software.
- When two records conflict (e.g., guest-TLS prerequisites, font-cache status,
  settings live propagation), the newest entry wins; this file flags each such
  conflict explicitly.

### Gaps
- No machine-readable current-status index exists; the reader must reconcile
  dates across `stage-status.md`, `agent-coordination.md`, and the
  `release-qualification` reports manually.
