# Stage 11 thread generation exhaustion — open correction

## Finding

`thread::create` selects every `State::Free` slot. Join, abort and owner reaping
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

No production kernel source changed for this experiment. The current
preservation/phase-repeat source manifest remains frozen.

## Required integration gate

After the frozen loader baseline is accepted, implement permanent retirement
of exhausted slots without reducing ordinary capacity or recycling stale
authority. Add tests against the actual registry source for all three release
paths, final available generation, exhausted capacity, owner isolation and
ordinary slot reuse. Preserve the existing thread/syscall ABI and teardown
ordering. Run warning-denying host/kernel Clippy, host tests, Stage 11.2 on
1/2/4 CPUs and inherited acceptance before accepting the correction.

The candidate is not an accepted production fix. Stage 11 readiness remains
open until integration and runtime preservation pass.
