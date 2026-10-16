# ATA PIO flush correction — release candidate, acceptance pending

Scope: the next stages 1–5 durability correction for the user's stages 1–10
desktop/laptop release preparation. Based on combined tree `35d59de9` in
isolated branch `codex/foundation-ata-flush-20261016`. No new release version
or qualified binary is declared here.

## Defect and implementation

AtaPio inherited BlockDevice's successful no-op flush. The software cache
wrote dirty sectors, then called that default, so its successful return did
not establish device-cache durability. The driver now overrides flush and
uses a safe, allocation-free non-data command sequencer shared with host tests.
It selects EAh only when valid IDENTIFY word 83 advertises both 48-bit and
extended-flush support; otherwise it issues E7h and propagates any rejection.
It never infers success from absent or unknown cache capability information.

The sequencer waits for idle, selects the primary master with cleared low
device bits, settles, checks selected-device idle, issues exactly one command,
settles and validates completion. BSY dominates other status bits; final ERR,
DF, unexpected DRQ or an absent/floating bus fails. Pre-command stale error
bits cannot substitute for completion of this new command. A timeout/error
is returned without blindly reissuing an uncertain command; explicit retries
must reacquire idle. Both tick expiry and a stopped-timer spin bound apply,
including a timer that advances once and then stalls. Tick wrap is checked.

Primary reference: [T13/2161-D revision 5, ACS-3 draft](https://read.seas.harvard.edu/cs161/2019/pdf/ata-atapi-8.pdf),
sections 7.10–7.11 and 7.12.7.40. This specification permits flush durations
over 30 seconds. Existing five-second/50-million-stalled-spin limits were NOT
increased; slow physical devices can fail closed and are not qualified here.

## Ownership and safety review

No syscall, capability, userspace-pointer retention, new handle/object table,
allocation or lock is added. AtaPio stays privately constructed and owned by
the existing ranked channel mutex. The adapter uses four documented unsafe
calls to existing port-I/O/delay primitives; sequencing is safe Rust, with no
new assembly. It does not sleep, switch tasks or release channel ownership
mid-command. The outer existing block worker retains its event/completion
and cancellation semantics. Polling under the inherited channel IRQ lock is
not new interrupt-driven hardware qualification. Controller reset/recovery,
long device stalls and native power-loss tests remain open.

## Tests and gate

Nineteen actual-source host tests pass after exercising, rather than
suppressing warnings for, imported block/cache contracts. They cover command
selection/order, stale status, every final error, absence, busy/data phases,
deadlines, stopped/progress-then-stopped clocks, wrap, explicit retry and
software-cache/partition flush/error propagation. Software write failure must
retain dirty data and issue no flush; an empty cache must forward a retry.
First build failure (dependency DNS) and warning-denying test compile failure
are retained beside the passing result in
`C:/WovenHat-OS/audit-artifacts/ata-flush-20261016/`.

The new independently guarded `stage1-5-ata-flush-test` counts actual issued
and successfully completed commands only in that test build. Its self-test
calls the real cached ATA device and requires exactly one additional command
and completion, even when the software cache is empty. Storage harness option
`--ata-flush` requires that marker plus every original live FAT32/SMP gate.
Its output path is distinct. Negative host harness tests remove every marker
to reject incomplete evidence. No existing assertion or timeout is weakened.

Host/production/affected-feature lint, all host tests, real 1/2/4-CPU debug
and optimized ATA gates and full inherited preservation remain required before
software acceptance. The preceding main validator chain independently failed
live DNS after 60 passing stress boots; all 861 inputs matched. That failure
is not waived and cannot be relabelled as success for this patch.

Focused checkpoint: warning-denying host all-targets and new ATA-feature
Clippy pass, and all 32 Python tests pass. The first 1-CPU real ATA debug
boot passed (exit 33), retaining every FAT32/SMP marker and the additional
issued/completed command marker. Its serial is copied to
`C:/WovenHat-OS/audit-artifacts/ata-flush-20261016/first-qemu-1cpu-serial.log`.
The initial caller looked for this serial under the shared Cargo target,
whereas harness evidence is relative to its checkout; the authoritative
worktree serial was subsequently copied explicitly and preserved. This
focused result does not close the remaining matrix or inherited acceptance.

## Explicit release limits

This candidate does not establish physical cache honesty, torn-sector or
power-loss recovery, general interrupt-driven PIO completion, arbitrary/full-
volume rollback, or desktop/laptop compatibility. The ignored metadata-import
flush error in storage.rs also needs a separate error-path correction/review.
The known randomized mmap/stack guard and live DNS failure remain release
blockers. Source integration and focused tests are not full finalisation.
