# Stage 11 dynamic dependency metadata audit — 2026-10-14

## Scope

This increment hardens the ELF dynamic metadata boundary before implementing
shared-library mapping. The parser now retains at most four `DT_NEEDED` string
offsets, checks every offset against the validated string table, and rejects
an over-capacity dependency list. The current loader explicitly rejects an
image with dependencies because it does not yet provide dependency namespace,
cross-image symbol scope, dynamic TLS, or shared-image teardown.

This is intentionally fail-closed. It does not claim shared-library support.

## Validation

- Freestanding kernel Clippy for `stage11-5-test`, `-D warnings`: passed.
- Host all-targets Clippy, `-D warnings`: passed.
- Complete `wovenhat-os` host test suite: passed.
- Stage 11.3, 1/2/4 CPUs: passed, exit 33.
  - `audit-artifacts/stage11.3-1cpu-1791979019677103300`
  - `audit-artifacts/stage11.3-2cpu-1791979074468240000`
  - `audit-artifacts/stage11.3-4cpu-1791979113345985100`
- Stage 11.4, 1/2/4 CPUs: passed, exit 33.
  - `audit-artifacts/stage11.4-1cpu-1791979244427396000`
  - `audit-artifacts/stage11.4-2cpu-1791979348256155100`
  - `audit-artifacts/stage11.4-4cpu-1791979401924884100`
- Stage 11.5, 1/2/4 CPUs: passed, exit 33.
  - `audit-artifacts/stage11.5-1cpu-1791978975094941300`
  - `audit-artifacts/stage11.5-2cpu-1791979454404979000`
  - `audit-artifacts/stage11.5-4cpu-1791979520620604600`

One earlier Stage 11.4 1-CPU attempt timed out during early boot before the
acceptance markers; its artifact remains at
`audit-artifacts/stage11.4-1cpu-1791979155491013700`. The same exact run was
rerun without changing assertions or timeouts and passed at the artifact listed
above.

## Remaining prerequisite

The next implementation slice must add a bounded `/lib` dependency namespace,
separate W^X mappings, exact-name cross-image symbol resolution, and ordered
teardown. Versioned symbols, lazy binding, and dynamic TLS remain unsupported
until separately designed and tested.
