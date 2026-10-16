# Stage 11 thread generation exhaustion — bounded correction accepted

## Final serialized acceptance — 2026-10-15

Session 79248 completed with exit 0 and
`=== THREAD GENERATION RUNTIME AND PRESERVATION: PASS ===`. The fresh attempt
`audit-artifacts/thread-generation-20261015/runtime-attempt-20261015-180920-929/`
passed Stage 11.2 on 1/2/4 CPUs, the exact full `RUN-STAGE10.7.ps1` launcher,
and Stage 10.8, 10.9, 11.1, 11.3, 11.4 and 11.5 on 1/2/4 CPUs. The launcher
evidence is `audit-artifacts/acceptance-20261015-181015-071/`; inherited stress
includes 10 one-CPU, 30 two-CPU and 20 four-CPU memory boots plus live network
and isolated asynchronous I/O gates. Host QA ends with its explicit pass marker.
All 135 original frozen source hashes matched at completion and independent
post-run inspection. No competing foreign test/compiler/QEMU process was found
at that inspection. Earlier interrupted attempts remain historical evidence,
not the basis for acceptance.

This accepts the permanent thread-record retirement correction committed in
`92959b3d`, including its actual-source host boundary regressions and inherited
runtime preservation. It does not resolve the separately documented negative
exit-result ABI collision, IPC rollover, compiler TLS, loader interoperability,
combined-tree integration, or hardware qualification. No release image is
newly qualified by this increment. The pending statements below describe the
retained investigation timeline, not the final acceptance decision.

## Finding

At the reviewed baseline, `thread::create` selected every `State::Free` slot.
Join, abort and owner reaping
advance its generation with checked addition but saturate at `u32::MAX`.
Once that generation is issued and retired, the next allocation can issue the
same slot/generation handle again. Stale handles can then address a later
thread. This contradicts the generation-safe lifecycle contract even though
ordinary low-generation teardown tests pass.

The reviewed source has SHA-256
`976c13aa42c7511c5979c97aca93b9e67770618253a8043b0e762c0c6489eab0`.
This is an existing Stage 11.2 defect, independent of signed library loading.

## Reproduction and candidate evidence

Artifacts are in `audit-artifacts/shared-library-integration-20261015/`:

- `thread-generation-repro.rs` copies the production registry and adds private
  host tests. A standard host mutex replaces only the IRQ-lock environment,
  and a minimal mapping type stands in for opaque resource payloads. These
  tests make no claim about hardware locking, paging or scheduler behavior.
- The tests seed all slots at generation `u32::MAX - 1`, allocate and retire
  through join, abort or owner reaping, and reject any reissued stale handle.
  All three fail against the copied production logic (exit 101), retained in
  `thread-generation-repro-all.log`. The earlier single-case failure is also
  preserved in `thread-generation-repro.log`.
- `thread-generation-candidate.rs` changes allocation to skip saturated
  generations. All three tests then pass, including complete exhausted-table
  refusal (`thread-generation-candidate.log`). Both host binaries compile
  with warnings denied.

No production kernel source changed for that experiment. The loader increment
has since passed its preservation gates and is committed as `454f1ff2`.

## Current correction

`Record::reusable` reserves `u32::MAX` as a permanent retirement sentinel,
and allocation applies that predicate under the existing rank-10 registry
guard. The last issued generation is `u32::MAX - 1`; each release path already
advances that to the sentinel. Saturated free records remain unavailable rather
than recycling stale authority. The 64-slot ordinary capacity, thread-handle
encoding, syscall numbers, ownership checks and full-table refusal ABI remain
unchanged. No new object, lock, allocation, unsafe code, wait or cross-CPU
publication path is introduced.

`tests/thread_registry.rs` now imports the actual production registry, not a
copied implementation. Its test-only IRQ-lock and opaque mapping doubles make
the registry state machine executable on the host. Before the fix, all three
exhaustion cases fail while ordinary owner/waiter reuse passes
(`thread-registry-before.log`, exit 101). After integration, those cases plus
full 64-slot capacity/reuse all pass (`thread-registry-after.log`). The earlier
copied-source stale-alias reproducer remains preserved separately.

The Stage 11.2 structural fixture also checks the retirement boundary and now
reclaims its final replacement record rather than leaving a test-owned live
entry behind. All existing assertions remain; the new cleanup requires both
a distinct replacement and a successful terminate/join.

Direct warning-denying host compilation, Cargo's five registry tests, and
test-target Clippy pass. Complete build, host all-targets Clippy, all host Rust
tests, sixteen `libwoven` tests, fifteen Python tests, production-kernel Clippy
and kernel feature Clippy for Stage 10.7, 10.8, 10.9 and 11.1–11.5 all pass.
Commands/results are retained under `audit-artifacts/thread-generation-20261015/`;
`host-qa.txt` ends with the explicit pass marker. Its manifest freezes 135
kernel/build/harness files, and all still match.

The runtime chain initially queued behind confirmed live Claude test processes
(PIDs 20996 and 24984 observed at launch). Its required sequence is Stage 11.2 on 1/2/4 CPUs,
the full `RUN-STAGE10.7.ps1` preservation chain, then Stage 10.8, 10.9, 11.1,
11.3, 11.4 and 11.5 on 1/2/4 CPUs. Failure stops the chain with its logs retained;
no assertions or timeouts were weakened. Runtime preservation remains pending.

Claude's original runner later recorded `release: PASS` and `DONE`; its release
report has 102 passing checks and no failures. Codex's first attempt began after
that runner exited, but Claude launched a new `run2015.py` acceptance chain
(PID 20780, start 2026-10-15 17:04:46). Codex stopped only its own verified
process tree rooted at PID 19632 rather than continue overlapping acceptance.
Session 22900 ended with exit 1 from that deliberate interruption; it is not
runtime acceptance. `runtime-chain.txt` and
`audit-artifacts/stage11.2-1cpu-1792076665295818400/` remain preserved; the
interrupted attempt's serial and QEMU logs are empty and prove no pass.

The stricter user-selected all-checkout rule and Codex's next-slot reservation
were synchronized into all three Claude worktree coordination documents.
The new runner `run-runtime-preservation.ps1` waits for the identified live
Claude chain, then for any remaining foreign compiler/test or QEMU process,
checks the original 135 hashes, and repeats the entire required sequence.
Every attempt uses a unique directory, so it cannot overwrite the interrupted
evidence. No foreign process/source was changed.

### Second handoff interruption — acceptance still open

Claude's `run2015.py` later ended after a 102-check release sub-chain pass;
its earlier inherited desktop installation/Quick Settings failures are retained
separately and prevent accepting the enclosing change. Codex's next attempt
started in `runtime-attempt-20261015-175941-447/`. Stage 11.2 reported passes
on 1/2/4 CPUs (exit 33), with evidence directories ending
`1792079981704042900`, `1792079993977286800`, `1792080011582643200`.
Full Stage 10.7 preservation then began.

A new Claude `run2015b.py` chain (PID 30304, start 17:59:57 local) overlapped
that handoff despite the written all-checkout reservation. Codex again stopped
only its own verified process tree, rooted at PID 29444 (start 17:10:16), and
session 4650 ended with exit 1. The partial attempt's logs remain intact; this
does not establish full runtime acceptance, and the complete sequence must be
repeated under actual serialization. No assertion, timeout or source hash was
changed to accommodate the overlap.

At that interruption no automatic restart remained queued. The user was asked to obtain Claude's
acknowledgement of the reserved slot because written notes have not prevented
two overlaps. Preserve the current source freeze until that coordination is
confirmed. This is a scheduling interruption, not a newly diagnosed kernel-test
failure or proof that the unaccepted correction is production-ready.

The user subsequently replied "Yes" to confirmation that Claude has agreed to
hold new acceptance chains. A fresh repeat now waits for the confirmed live
`run2015b.py` PID 30304 (start 17:59:57) and remaining foreign compiler/test/QEMU
processes. It checks the original source manifest and reruns the entire sequence
in another unique attempt directory. The overlapped attempt is not reused as
acceptance. No source or assertion was changed to resume the gate.

Fresh serialized session 79248 started
`runtime-attempt-20261015-180920-929/` after PID 30304 exited. Its Stage 11.2
1/2/4-CPU runs all passed with exit 33; retained evidence directories end in
`1792080561132871900`, `1792080580109722800`, `1792080595486127500`.
The subsequent live-process check found no foreign test/QEMU process. Full
Stage 10.7 preservation is now running, followed by the remaining matrices.
This is focused progress only: complete runtime acceptance is still pending.

During that wait, a byte-level check found `thread.rs` converted from LF to
CRLF (writer not established). Normalizing only line endings reproduced the
original frozen SHA-256 exactly. Explicit Unix-newline formatting restored
those exact bytes before any own runtime gate started; all 135 manifest entries
matched again. `line-ending-restoration.txt` retains both hashes and the
restoration command. The freeze was not replaced or relaxed.
The conversion recurred before runtime started. Git's system configuration
reports `core.autocrlf=true`; the actual writer remains unestablished. A narrowly
scoped `.gitattributes` rule now requests LF for `kernel/src/thread.rs` without
changing any user/system Git configuration or normalizing other frozen files.
The same exact original source hash and all 135 manifest entries match after
restoration; `git check-attr` confirms `text=set`, `eol=lf`.

### Build-space maintenance

Free space fell below 1 GB while other worktrees also compiled. The prescribed
routine's dry run found little idle incremental space, and its broad path
substring guard conservatively treated the main checkout as busy for work in
`C:\WovenHat-OS-wt*`. After this checkout's own host chain exited, explicit
process/path checks found no compiler/QEMU reference inside `C:\WovenHat-OS\`.
The literal `.cargo-build` target and all descendants were checked for reparse
points; only that own regenerable cache was removed (4,355,631,770 measured
file bytes). Sources, audit/gate evidence and `target/releases/0.8.0` were not
removed. Cargo will rebuild the cache. The preserved release image still hashes
to `6195a4721219a80b4e7775531a3657c7ce7e50a78648da53ca0b31b53ae03544`.
No foreign worktree cache or live QEMU process was modified.

Thirteen additional own `apps/<name>/target` caches were later measured at
about 2.2 GB and removed after validating every literal resolved target,
all parent/descendant reparse attributes and absence of own application builds.
These are the same regenerable per-app targets selected by the prescribed
routine's deep mode; sources, signed packages, release archives and gate/audit
evidence were preserved. The runtime chain was still waiting on Claude's live
test process. The packager's explicit per-app target policy explains repeated
dependency-cache growth; changing that policy requires separate build/output
qualification and was not mixed into the frozen kernel correction.

Before the queued runtime chain started, free space again fell below 3 GB.
The literal own `target/debug` and `target/x86_64-unknown-none` compiler-cache
directories were resolved and checked recursively for reparse points. No own
cargo/rustc/Clippy/QEMU process referenced this checkout, while the original
foreign runner PID and start time were revalidated. Only those two idle caches
were removed (2,087,889,718 and 715,611,196 measured file bytes). They are
regenerable, not test evidence. Free space rose to 5,555,716,096 bytes; the
retained release image again matched `6195a472…`. No gate, audit or release
directory was removed, and no foreign worktree was changed.

Later, with free space near 3 GB, the prescribed deep routine's dry run found
idle caches in `C:\WovenHat-OS-wt2`. Its literal root, `.cargo-build` and twelve
existing `apps/<name>/target` paths were resolved within that named worktree;
root/parent/descendant reparse checks passed. No compiler/Clippy/QEMU process
referenced it, and the routine independently rechecked that it was idle. Running
`python scripts/reclaim-build-space.py --min-free-gb 5 --deep C:\WovenHat-OS-wt2`
removed only those 13 regenerable compiler targets. The retained transcript is
`idle-wt2-cache-cleanup.log`; free space rose from about 3.0 to 6.5 GiB
(6,975,184,896 bytes observed). Unlike the earlier maintenance, this deliberately
reclaimed a different idle worktree's caches; it did not change its source,
audit/gate/release evidence or any live process. Its only tracked modification
remained the synchronized coordination document. The main retained release
image again matched `6195a472…`. No kernel freeze or test requirement changed.

## Required integration gate

Run warning-denying host/kernel Clippy, all host tests, Stage 11.2 on
1/2/4 CPUs and inherited acceptance before accepting the correction.

The bounded correction has passed the required integration gate above. Full
Stage 11 readiness remains open: negative exit-result encoding, compiler TLS,
loader interoperability, combined-tree integration and hardware qualification
remain separate prerequisites.
