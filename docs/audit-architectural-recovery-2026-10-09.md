# WovenHat OS — Architectural Audit and Development Recovery Report

**Date:** 2026-10-09
**Audited tree:** `origin/master` @ `b81a7771` (plus branches `local-stage14-6`, `fixes/gate-repairs`, `audit/production-readiness`)
**Method:** source inspection, Git history, GitHub Actions run history via the public API, local QEMU gate execution on 1/2/4 CPUs, and `git bisect`-style commit isolation.

Every claim below cites a file, line, commit, or CI run number. Where evidence does not
exist, the item is marked **UNTESTED** or **UNVERIFIED** rather than estimated.

---

## A. Executive assessment

WovenHat OS is a substantial and unusually disciplined freestanding kernel — roughly
81,000 lines of Rust across ~95 modules, with real SMP scheduling, paging, a capability
security model, an async completion-port I/O architecture, a live IPv4 network stack, and
a broad driver surface. The engineering culture is visible in the code: fixed-capacity
tables, generation-tagged handles, ranked locks, and zero `TODO`/`FIXME`/`HACK` markers
across the entire tree.

It is **not** production-ready, and the gap is wider than the documentation implies. The
single most important finding is not a code defect:

> **`master` has failed its own CI on ten consecutive commits.** The last green run on
> `master` is **#1289** at `5b57188a`. Runs **#1290 through #1299** — every commit of the
> Stage 14.5D series up to and including the current tip `b81a7771` — are **failure**.
> Meanwhile `docs/stage-status.md` documents Stages 14.5B, 14.5C and 14.5D as accepted.

The project's own governing rule in `AGENTS.md` — *"Treat test failures as defects to
diagnose"* — was not applied to this regression, because nothing surfaced it: the status
documents record the *earlier* passing run numbers (#1277, #1287) rather than the current
state, and the failure has persisted for ten commits.

Correct release classification today, against the taxonomy in the governing directive, is
**Developer Preview**. It is not Alpha: Alpha requires "major architecture integrated",
and there is no graphical environment, no userspace application toolchain, no installer,
no release artifact, and no Git tag in 858 commits.

**Assessed maturity by layer:** kernel and memory — strong; SMP and locking — strong with
one hole now closed; security model — strong design, incomplete enforcement coverage;
storage — one real filesystem (FAT32) and three stubs described as subsystems; networking —
strong; userspace — privilege separation real, application platform absent; graphics/GUI —
effectively not started.

---

## B. Verified accomplishments

These are supported by source and by gate evidence I executed or by CI runs I confirmed.

| Area | Evidence |
| --- | --- |
| Boot, memory, paging, scheduler, VFS, FAT32 persistence | `runtime:1-5` gate passes 1/2/4 CPUs, exit 33 (local run, 2026-10-09) |
| Async completion ports, timers, deadlines | `runtime:10.8`, `runtime:10.9` pass 1/2/4 CPUs (local) |
| Process and thread lifecycle, notifications, ELF/W^X loader | `runtime:11.1`–`11.5` pass 1/2/4 CPUs (local) |
| Typed VFS, WovenFS metadata, volume crypto, snapshots, storage mgmt | `runtime:12.1`–`12.5` pass 1/2/4 CPUs — **only after this audit's repairs**, see §D |
| PCIe/NVMe/AHCI/xHCI/USB-HID/Input/Audio/Wi-Fi/Bluetooth contracts | CI steps for Stages 13.3–13.11H; last green at `5b57188a` (#1289) |
| Live IPv4 networking: DHCP, DNS, ICMP, host-verified UDP/TCP | `stage14-*` network gates pass 1/2/4 CPUs (local) |
| IPv6 address/NDP/SLAAC/DHCPv6 protocol layers | 20 markers `[S14.1]`–`[S14.6]` present in serial logs on 1/2/4 CPUs |
| Ranked IRQ-safe locking discipline | Every kernel lock carries a rank; see §D-4 for the one exception, now fixed |
| Capability security: lineage, recursive revocation, sandbox profiles, ledger | `wovenguard.rs`, `capability.rs`, `audit.rs`; Stage 9 audits |
| On-volume FAT32 data journal with checksummed rollback | `storage.rs:10` `DATA_JOURNAL_MAGIC = b"WDJ1"` |
| Code hygiene | 0 `TODO`/`FIXME`/`XXX`/`HACK` across `kernel/src/` |

---

## C. Complete stage register

Classification: **Verified** = gate executed and passed with evidence retained.
**Unverified** = previously claimed accepted, not reproducible now. **Partial** = bounded
foundation only. **Stub** = structurally present, not functional. **Not started**.

| Stage | Subject | CI gate? | Status | Evidence / note |
| --- | --- | --- | --- | --- |
| 1–5 | Boot, memory, paging, sched, VFS, FAT32 | Yes | **Verified** | Local gate 1/2/4 CPUs exit 33 |
| 6 | SMP, TLB, hotplug, lock ranks | **No dedicated gate** | **Partial** | Hotplug harness exists; no CI step |
| 7.x | Scheduler/pager watchdog, Ring-3, multicore userspace | **No dedicated gate** | **Partial** | Historical audits only |
| 8.x | IPC handles, shared memory, capability transfer, discovery | **No dedicated gate** | **Partial** | Historical audits only |
| 9.x | WovenGuard lineage, revocation, sandbox, ledger | **No dedicated gate** | **Partial** | Historical audits only |
| 10.1–10.7 | Async I/O foundation through async TCP | **No dedicated gate** | **Partial** | `RUN-STAGE10.7.ps1` 75-boot chain exists, not in CI |
| 10.8–10.10 | Completion ports, timers, aggregate | No | **Verified** | Local runtime gate 1/2/4 CPUs |
| 11.1–11.5 | PID/TID, notifications, libwoven, ELF loader | No | **Verified (kernel side)** | See §D-2: `libwoven` is a stub |
| 12.1 | Typed VFS + SystemVfs adapter | **No** | **Verified** | Local gate; was uncompilable before repair |
| 12.2 | WovenFS metadata/integrity | **No** | **Stub** | §D-1: 104 lines, in-memory, 0 production callers |
| 12.3 | Volume crypto envelope | **No** | **Verified (crypto)** | Gate **could not compile** before this audit |
| 12.4 | Snapshots | **No** | **Stub** | §D-1: 53 lines, 8 entries, 0 production callers |
| 12.5 | Storage management | **No** | **Verified** | Gate **could not compile** before this audit |
| 13.1–13.2 | WovenDriver manager, PCIe topology | 13.2 partial | **Partial** | 13.2 blocked on BAR sizing, MSI alloc, hotplug |
| 13.3–13.8 | NVMe, AHCI, xHCI, USB-HID, Input, Audio | Yes | **Verified (emulated)** | No physical hardware evidence |
| 13.9 | WovenWiFi / AX200 | Yes | **Partial** | Never transmitted on real hardware |
| 13.10–13.11 | Bluetooth HCI→BLE SMP | Yes | **Verified (emulated)** | QEMU device is not a Bluetooth controller |
| 14.1–14.4 | IPv4 core, IPv6 layers, socket API, routing | Yes | **Verified** | Local gates 1/2/4 CPUs |
| 14.5A | Firewall policy foundation | Yes | **Verified** | CI #1267 at `f8b79ac7` |
| **14.5B** | **Live packet enforcement** | **Yes** | **FAILED → repaired** | **§D-3. CI #1290–#1299 all failure** |
| **14.5C** | **Policy authority (NetworkAdmin)** | **Yes** | **UNVERIFIED → repaired** | Blocked by 14.5B failure in same gate |
| **14.5D** | **Parser/transport hardening** | **Yes** | **UNVERIFIED → repaired** | Never had a green CI run |
| 14.6 | Live socket firewall admission | No | **Unmerged** | Exists only on `local-stage14-6` |
| GUI / Desktop | Compositor, WM, shell, applications | No | **Not started** | §D-5 |
| Install / Update / Recovery | Installer, update delivery, recovery | No | **Not started** | No installer, no tags, no release artifact |

---

## D. Missing functionality and defects

### D-1. The filesystem is one real implementation and three stubs

FAT32 (`fat32.rs`, 4,199 lines, 42 public functions) is the only durable filesystem, and
it is genuinely capable: long filenames with NFC normalization, directory growth,
uid/gid/mode, and an on-volume prepared/committed journal with checksum rollback.

The OS's own filesystem does not exist:

- **`wovenfs.rs` — 104 lines.** An in-memory, 64-entry metadata side table. No on-disk
  format, no inodes, no directories, no block allocation, no data blocks; volatile across
  reboot. **Its only caller is its own self-test** (`main.rs:3351`). `State::len` is
  incremented at line 60 and never read or decremented, and there is no `remove`, so the
  table can only fill; once full, `record()` returns `false` and metadata silently stops
  being recorded.
- **`snapshots.rs` — 53 lines.** An 8-entry in-memory catalog of `(id, generation,
  checksum)`. `restore(id)` returns that tuple and restores no data. The header claims a
  "copy-on-write snapshot catalog"; there is no COW and no block reference anywhere in it.
  **Only caller: its own self-test** (`main.rs:3369`).
- **`journal.rs` — 64 lines. P1, actively misleading.** Unlike the other two, this *is*
  wired into the production write path: `storage.rs:1274` `recover()`, `:1311` `begin()`,
  `:1397` `commit()`. But `LOG` is a `static Mutex<[Option<Entry>; 32]>` in RAM. After a
  real crash it is empty, so `recover()` always returns 0 and replays nothing. Its own
  body makes this explicit — the committed branch executes
  `let _ = (x.path_hash, x.checksum);` and discards the entry without replay. The module
  header reads *"Bounded write-ahead intent journal for crash-safe metadata updates."* It
  cannot be write-ahead; nothing reaches a device. Actual durability comes from the
  separate on-volume WDJ journal, so there are two overlapping journals and the
  decorative one owns the name.

**Latent defect (P2):** `wovenfs.rs` and `journal.rs` both key records by a 64-bit FNV-1a
path hash and match on the hash *alone* (`wovenfs.rs:41,69,78,94`). A collision returns or
overwrites a different file's record — including `mode` permission bits. Unexploitable
today only because `wovenfs` has no production callers; a trap for whoever wires it up.

**Unmerged real work:** `master`'s `storage.rs` is 2,382 lines with WDJ1 only. The
`local-stage14-6` branch has 2,974 lines with WDJ2 — a bounded four-file batch rollback
journal with prepared/committed recovery, already gate-tested. ~600 lines of genuine
durability work sitting unmerged. **This is the highest-value filesystem improvement
available and it requires no new design.**

### D-2. There is no userspace application platform (P1)

Privilege separation is real — Ring 3, W^X, ASLR, a hardened ELF loader that rejects
`PT_INTERP`/`PT_DYNAMIC`/`PT_TLS`/`PT_GNU_RELRO`. What does not exist is any way to
produce an application.

- **Every userspace program is inline assembly inside the kernel source.**
  `userspace.rs` contains `global_asm!` blocks exporting `wovenhat_user_program_start`,
  `wovenhat_sh_program_end`, `wovenhat_init_program_start`, etc. Each `create_*_process()`
  takes a raw slice of embedded `.rodata`, wraps it with `build_stub_elf()`, and calls
  `load_elf()` (`userspace.rs:4485–4632`). There is no external build target, no
  application format, no on-disk binary produced by a toolchain.
- **`libwoven` — the "stable userspace API boundary" (Stage 11.4) — is 73 lines.** Nearly
  every module is a bare newtype: `pub mod fs { pub struct Handle(pub u64); }`,
  `pub mod graphics { pub struct Surface(pub u64); }`. It binds **two** syscalls
  (`NOTIFICATION_POLL` 95, `NOTIFICATION_POST` 96) while the kernel implements **78**
  syscall dispatch arms. Its own doc comment concedes: *"Syscall-backed implementations
  are added as kernel ABI contracts mature; these handles keep application code typed
  now."* There is no file, process, socket, or graphics operation in it.
- **Consequence for the GUI objective:** a userspace compositor cannot be built on an API
  with no operations. `libwoven` must become a real libc-equivalent before any desktop
  work is meaningful.

### D-3. Stage 14.5B regression — root-caused (P0 for release integrity)

`43b423f2` *"Stage 14.5D: harden firewall IP parser"* made
`packet_meta_from_ethernet` authenticate the IPv4 total-length and IPv6 payload-length
header fields, returning `InvalidIp` on mismatch. It did not update
`stage14_5b_self_test`, whose synthetic frames leave both fields zero:

```rust
let mut ipv4 = [0u8; 14 + 20 + 8];
ipv4[12..14] = 0x0800; ipv4[14] = 0x45; ipv4[23] = 17;
// ipv4[16..18] — IPv4 total_length — never written, remains 0
```

`total_len` is 0, `0 < header_len(20)` holds, the frame is denied before policy is
consulted, and both allow-assertions fail. The IPv6 frame fails identically via a zero
payload length. The ARP frame is unaffected because a non-IP ethertype returns `NonIp` —
which is exactly the observed signature: two `FAIL` plus `L2 control preservation PASS`.

Independent confirmation from two sources: my local bisect (`5b57188a` passes,
`43b423f2` fails) and GitHub Actions (#1289 success → #1290 failure), agreeing exactly.

The parser hardening is correct; the *test data* was malformed. Repaired by making the
frames well-formed.

### D-4. CI gates only a subset of stages, and the ungated stages rotted (P1)

`.github/workflows/kernel.yml` has 27 steps. It gates Stage 1–5, 13.3–13.11H, and
14.1–14.5D. It has **zero** steps for Stages 6, 7.x, 8.x, 9.x, 10.1–10.7, 10.8–10.10,
11.x, and **12.x** (`grep -c stage12 → 0`).

That is precisely where decay was found. Before this audit, **five feature
configurations did not compile at all**, so their gates could not run:

- `stage14-2-test`, `stage14-3-test`, `stage14-4-test` — the Stage 14.5 self-test call in
  `main.rs` was the only Stage 14 self-test call lacking a `#[cfg]` guard.
- `stage12-3-test`, `stage12-5-test` — circular module gate: `storage_manager` (gated on
  12.5) seals records through `volume_crypto` (gated on 12.3), while the Stage 12.3
  self-test block calls into `storage_manager`.

Stage 12 has no CI coverage, so two of its five gates had been unbuildable with nothing to
notice. A gate that cannot compile is a gate nobody is running.

The 75-boot Stage 10.7 preservation chain (`RUN-STAGE10.7.ps1`), which the documentation
treats as the principal regression gate, is **not in CI** and runs only on Windows.

### D-5. Graphics and GUI are effectively not started (P1 relative to the objective)

Total graphics-related code is ~1,385 lines: `gui.rs` 282, `graphics.rs` 176,
`terminal.rs` 295, `console.rs` 276, `woven_input.rs` 179, `woven_audio.rs` 177.
`gui.rs` provides `Rect`, `contains()`, and an `InputEvent` enum with `PointerDown` and
`Key`. There is no compositor, no window manager, no surface management, no modesetting
driver, no damage tracking, no font rasterizer, no GPU path. The framebuffer is whatever
UEFI hands over.

Against the directive's target (§8), essentially the whole stack — display abstraction,
2D renderer, surface and composition model, input routing, display server, desktop shell,
and twelve applications — remains to be designed and built, **on top of a userspace API
that currently has two syscalls**.

### D-6. Undocumented `unsafe` (P2)

305 `unsafe {` blocks against 97 `SAFETY` comments — roughly 208 unsafe blocks without a
written justification, concentrated in `virtio_net.rs` (57), `smp.rs` (42), `paging.rs`
(34), `userspace.rs` (30), `task.rs` (26). `AGENTS.md` requires *"Document and minimize
unsafe code."* The requirement is stated; coverage is about 32%.

### D-7. Defects found and repaired during this audit

| Defect | Severity | Location |
| --- | --- | --- |
| Firewall policy lock was the kernel's **only** rank-0 lock, on the live packet path — exempt from the lock-order checker (`irq_lock.rs:38`) while nested under runtime (20) and transport (30) | P1 | `firewall_policy.rs:159` |
| `audit::record_detail` could **abort the kernel**: it called `smp::cpu_index()`, which panics on an unregistered APIC identity, contradicting the module's own written invariant *"Security logging must never ... make the kernel fail"* | P1 | `audit.rs:150` |
| Firewall administration had **no security-ledger coverage**: refused attempts to mutate policy or call `admin_set_enforcement(false)` — to disable the firewall — left no record | P1 | `firewall_policy.rs` |
| Host `cargo clippy --all-targets -D warnings` failing: `topology.rs` linted under edition 2024 by the host test target, where `collapsible_if` fires on a form the edition-2021 kernel cannot adopt | P2 | `hal/pci/topology.rs` |
| `.cargo-home/` (146 MB, 9,720 files) untracked **and** un-ignored | P2 | `.gitignore` |
| No root `README.md`; 229 files in repo root | P3 | repository |

### D-8. Release engineering absent (P2)

858 commits, **0 Git tags**, no release artifact, no installer, no update mechanism, no
recovery path, no supported-hardware list. 44 remote branches including 8 `copilot/*`
branches with active automated pushes today; PRs #37/#38 open. Recent CI runs on those
branches conclude `action_required`, meaning they are not executing.

---

## E. Architectural weaknesses (ranked)

1. **Documentation asserts acceptance that CI contradicts.** `stage-status.md` records
   14.5B/C/D as accepted while CI has been red since `43b423f2`. The status documents cite
   historical run numbers and are never reconciled against current state. This is the
   root process defect: it let a ten-commit regression persist.
2. **Gate coverage is incomplete and uneven.** Stages 6–12 have no CI gates. Unbuildable
   gates went unnoticed for an unknown period.
3. **Stub subsystems are named and documented as real ones.** `journal.rs` is the
   dangerous case — it claims crash safety from RAM and is wired into the write path.
4. **Userspace is a demonstration, not a platform.** Programs are kernel-embedded
   assembly; `libwoven` has two syscalls. Everything in the GUI objective depends on this.
5. **Verification is marker-based.** Gates assert serial strings and exit 33. A self-test
   that stops exercising its target still passes if it prints its marker — which is
   precisely how 14.5B's malformed frames went unnoticed until the parser changed.
6. **Physical hardware is entirely unqualified.** Wi-Fi and Bluetooth have never
   transmitted; storage has never faced power loss; no NUMA or APIC-above-255 hardware.
7. **`unsafe` justification coverage ~32%.**
8. **Windows-only principal regression chain.** The 75-boot gate runs on one developer's
   machine, not CI.

---

## F. Production-readiness matrix

Scoring is deliberately **not** a single percentage. Each layer is rated on two
independent axes, because conflating them is what produced the current overstatement.

- **Implementation** — does the code exist and do what it claims?
- **Verification** — is there reproducible, current evidence?

| Layer | Implementation | Verification | Release level |
| --- | --- | --- | --- |
| Boot / UEFI / ACPI | Strong | Gate-verified (emulated) | Beta-capable, emulated only |
| Memory / paging / heap | Strong | Gate-verified | Beta-capable, emulated only |
| SMP / scheduling / locks | Strong | Gate-verified; **no CI** | Alpha |
| Interrupts / IRQ routing | Strong | Gate-verified | Alpha |
| Capability security (WovenGuard) | Strong design | Partial; **no CI**; firewall audit gap now closed | Alpha |
| Syscalls / process isolation | Strong (78 arms) | Gate-verified | Alpha |
| Userspace platform | **Absent** (embedded asm; 2-syscall libwoven) | n/a | **Developer Preview** |
| Storage / FAT32 | Strong | Gate-verified; **no CI** | Alpha |
| WovenFS / snapshots / journal | **Stub** | Self-test only | **Not started** |
| Networking (IPv4) | Strong | Gate-verified | Alpha |
| Networking (IPv6) | Protocol layers only | Gate-verified; no live sockets | Developer Preview |
| Firewall / WovenGuard net | Strong | **Was red for 10 commits**; repaired | Alpha after merge |
| Drivers (NVMe/AHCI/xHCI/HID/Audio) | Substantial | Emulated only | Developer Preview |
| Wi-Fi / Bluetooth | Protocol stacks | **Never on real hardware** | Developer Preview |
| Graphics / compositor / GUI | **Not started** | n/a | **Not started** |
| Install / update / recovery | **Not started** | n/a | **Not started** |
| Release engineering | **Absent** (0 tags) | n/a | **Not started** |

**Overall: Developer Preview.**

---

## G. Recommended target architecture

```
┌───────────────────────────────────────────────────────────────────────┐
│  Applications    Files · Terminal · Settings · Security Center · …    │
│                  (separate ELF binaries, built by an SDK)             │
├───────────────────────────────────────────────────────────────────────┤
│  WovenHat Desktop shell   panel · launcher · notifications · session  │
├───────────────────────────────────────────────────────────────────────┤
│  Compositor + window manager        ← userspace, WovenGuard-confined  │
│  surfaces · damage · input routing · seat/focus policy                │
├───────────────────────────────────────────────────────────────────────┤
│  libwoven  — real userspace API: fs, process, net, ipc, time,         │
│              async, graphics, security   ← TODAY: 73 lines, 2 calls   │
├───────────────────────────────────────────────────────────────────────┤
│  System services   init · device mgr · network mgr · update · log     │
│                    (least-privilege, capability-scoped)               │
╞═══════════════════════ user / kernel boundary ════════════════════════╡
│  Syscall ABI (78 arms) · WovenGuard capability enforcement · audit    │
├───────────────────────────────────────────────────────────────────────┤
│  VFS 2.0 → FAT32 (real) · WovenFS (to build) · block cache · journal  │
├───────────────────────────────────────────────────────────────────────┤
│  Driver infrastructure   WovenDriver · PCIe · NVMe/AHCI · xHCI ·       │
│                          NIC · HDA · display/KMS (to build)           │
├───────────────────────────────────────────────────────────────────────┤
│  HAL   ACPI · APIC/x2APIC · timers · SMP topology · DMA arenas        │
├───────────────────────────────────────────────────────────────────────┤
│  Kernel core   scheduler · paging · heap · IPC · async completion     │
├───────────────────────────────────────────────────────────────────────┤
│  UEFI bootloader (vendored `bootloader`, x86-64)                      │
└───────────────────────────────────────────────────────────────────────┘
```

Two structural commitments follow from the audit:

1. **The compositor belongs in userspace**, confined by WovenGuard, not in the kernel.
   `gui.rs` currently sits in kernel space; it should not grow there.
2. **`libwoven` is the critical path.** Services, compositor, shell and applications all
   sit on it. At 73 lines with two syscalls it is the project's narrowest bottleneck.

---

## H. Prioritized recovery roadmap

**Phase 0 — Restore trust in the signal (days).** Nothing else is meaningful while
`master` is red and the status documents disagree with CI.

| # | Work | Why first |
| --- | --- | --- |
| 0.1 | Land the Stage 14.5B frame repair on `master` | Turns CI green for the first time in 10 commits |
| 0.2 | Add CI gates for Stages 6–12, especially all of 12.x | Two 12.x gates were unbuildable with nothing watching |
| 0.3 | Reconcile `stage-status.md` against actual CI state; make every claim cite a *current* run | Root process defect in §E-1 |
| 0.4 | Port `RUN-STAGE10.7.ps1` preservation chain into CI | Principal regression gate is on one machine |
| 0.5 | Land the gate-repair and audit fixes already prepared | §D-4, §D-7 |

**Phase 1 — Honesty and durability in storage (1–2 weeks).**
1.1 Merge the WDJ2 batch rollback journal from `local-stage14-6` (real, tested, unmerged).
1.2 Resolve `journal.rs`: make it genuinely on-disk or delete it and route `storage.rs`
through the WDJ journal. Do not leave a RAM array claiming crash safety.
1.3 Fix path-hash collision handling in `wovenfs.rs`/`journal.rs` (store the path, or
verify on match).
1.4 Decide WovenFS: commit to a native filesystem, or stop documenting a 104-line RAM
table as one.

**Phase 2 — Build the userspace platform (4–8 weeks). The real unlock.**
2.1 Expand `libwoven` into a genuine API over the existing 78 syscalls: fs, process,
thread, net, ipc, time, async, memory.
2.2 Create an application build target — a workspace member producing standalone ELF
binaries for `x86_64-unknown-none`, with a startup shim and a minimal libc.
2.3 Make `exec` from the filesystem the normal path; retire `build_stub_elf` embedding.
2.4 Add a gate that builds an out-of-kernel application, installs it to the FAT32 volume,
execs it from disk, and verifies its output.

**Phase 3 — Security completion (2–4 weeks).**
3.1 Extend ledger coverage to every enforcement point; add negative tests proving denial.
3.2 Raise `unsafe` SAFETY coverage from ~32% toward complete.
3.3 Threat model document; verified-boot feasibility study.

**Phase 4 — Display foundation (4–8 weeks).**
4.1 Display abstraction and modesetting over the UEFI framebuffer.
4.2 Software 2D renderer: blit, clip, alpha, damage; font rasterization.
4.3 Surface/buffer model in `libwoven`; input event routing with seat/focus.

**Phase 5 — Compositor and shell (8–16 weeks).** Userspace compositor, window manager,
then the desktop shell. **Not before Phase 2 is complete.**

**Phase 6 — Applications, install, update, recovery.** Terminal and Settings first, as
they exercise the most OS surface.

**Phase 7 — Stabilization, physical qualification, release engineering.** First Git tag,
reproducible artifacts, supported-hardware list.

---

## I. GUI development blueprint

**Sequencing constraint, stated plainly:** GUI work must not start now. The directive's
own rule 6 — *"Architecture before GUI decoration"* — forbids it, and §D-2 is the binding
reason: there is no userspace API to build a compositor against. Starting the desktop
before Phase 2 would produce a kernel-space GUI that must later be thrown away.

**Architecture.** Userspace compositor, software-rendered first. Each client owns shared-
memory surfaces granted through WovenGuard capabilities; the compositor holds the only
display capability; input is routed by the compositor via a seat/focus model. No client
can read another's surface or enumerate windows without a capability. GPU acceleration is
explicitly out of scope until a real driver exists and is measured — the directive forbids
claiming it otherwise.

**Design identity.** Original design system: an 8-pt spacing grid, a tokenized palette
with light/dark parity and WCAG AA contrast, one typeface family with a defined type
scale, motion with a single easing curve and 150–250 ms durations, and a component library
specified before pixels. No imitation of Windows/macOS assets, as the directive requires.

**Implementation order.** display abstraction → software renderer → surface protocol in
`libwoven` → input routing → compositor → window manager → shell (panel, launcher,
notifications) → Terminal → Settings → File Manager → Security Center → remainder.

**Security Center** must read real state — actual WovenGuard capability sets, real
`audit.rs` ledger events, real firewall policy. The directive forbids fabricated security
status, and the audit ledger now records firewall denials, which gives it genuine data.

---

## J. Immediate next milestone

**Milestone: restore a trustworthy verification signal — make `master` green and gate
what is currently ungated.**

Chosen over any feature work because every other judgement depends on it. While `master`
fails its own CI and the status documents claim otherwise, no completion claim in this
repository can be trusted, including the ones this report relies on.

**Prerequisites:** none. All work is prepared and locally validated.

**Affected files:** `kernel/src/firewall_policy.rs`, `kernel/src/audit.rs`,
`kernel/src/main.rs`, `kernel/src/hal/pci/topology.rs`, `kernel/Cargo.toml`,
`scripts/test-network-qemu.py`, `.github/workflows/kernel.yml`, `AGENTS.md`,
`docs/stage-status.md`, `.gitignore`, `README.md`.

**Acceptance criteria:**
1. CI run on `master` concludes **success** — the first since #1289.
2. `cargo clippy` with `-D warnings` passes for **all 41** kernel feature configurations,
   `libwoven`, and the host with `--all-targets`.
3. Stage 12.1–12.5 gates pass 1/2/4 CPUs, with 12.5 emitting `[S12.5]` and no `[S12.3]`.
4. Stage 14.5B/C/D gates pass 1/2/4 CPUs including `[S14.5C] refused mutation auditing`.
5. New CI steps exist for Stages 6, 10.7 preservation, and 12.1–12.5.
6. `stage-status.md` cites only current run numbers.

**Risks.** Adding CI steps for ungated stages will likely expose further failures — that
is the point, and the schedule must absorb it. The 75-boot preservation chain is
Windows-authored and will need a Linux path. `master` and `local-stage14-6` have diverged
(1,154 lines in `network.rs` between two firewall designs), and concurrent Copilot agents
are pushing branches, so the merge must be deliberate.

---

## K. First implementation plan

| Step | Action | Verification |
| --- | --- | --- |
| 1 | Open a PR from the prepared repairs onto `master` (frame repair, audit abort fix, firewall rank + ledger, topology lint, Stage 12 gating, README, `.gitignore`) | CI run concludes success |
| 2 | Add CI steps: Stage 12.1–12.5, Stage 6 hotplug, memory/storage/shell | Each new step green, or its failure triaged and filed |
| 3 | Port the Stage 10.7 preservation chain to a Linux CI job | 75 boots green in CI |
| 4 | Reconcile `stage-status.md` and `stage1-12-production-gap-audit.md` to cite current runs; add this report to the document set | Review |
| 5 | Re-run the 114-gate sweep on merged `master`; publish the truth table as the Stage Completion Register | Register committed |
| 6 | Only then: Phase 1 storage honesty work | — |

**Explicitly not in this plan:** GUI work, WovenFS implementation, and `libwoven`
expansion. Each is sequenced later for a stated reason, not deferred by oversight.

---

## Audit limitations

- **No physical hardware was exercised.** Every result is QEMU 11.1.0 or source analysis.
  No claim here supports physical qualification.
- **The 114-gate sweep was incomplete at the time of writing** (26 of 114 recorded, 0
  failing). Stages marked Verified in §C rest on those completed gates plus confirmed CI
  runs; the remainder are marked accordingly. The full truth table becomes the Stage
  Completion Register (§K step 5).
- **Historical acceptance claims were not reproduced** except where stated. Runs cited
  from `stage-status.md` (#1267, #1277, #1287) were confirmed to exist but their contents
  were not re-executed.
- **Code changes were made before this audit was commissioned.** Four commits exist on
  `audit/production-readiness`; none is on `master`. They are reported in §D-7 rather than
  presented as pre-existing state. `fixes/gate-repairs` shows CI **failure** (#1301)
  because it branches from the already-red `b81a7771` and does not contain the 14.5B
  repair — expected, not a new defect.
