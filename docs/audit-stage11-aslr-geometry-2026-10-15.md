# Production-ASLR geometry — source finding, correction not implemented

This read-only review ran during the frozen compiler-TLS chain (session
77593). It changes no kernel source, executes no second build/QEMU chain and
does not certify a planner. Source baseline: `e58b803b`.

## Authoritative source relationships

`userspace::aslr_stack_base` subtracts a draw of 0–63 pages from offset
`0x1f0000`. `aslr_mmap_base` adds a draw of 0–79 pages to `0xe0000`.
`entropy::random_range` has an exclusive upper bound. Config preserves sixteen
anonymous slots, each with a 64 KiB stride/maximum. `UserStack` reserves a
4 KiB guard immediately below its base and an 8 KiB stack above it.

The two draws are independent. A direct enumeration of all 5120 stack/mmap
pairs, without compiling or changing source, gives:

| Geometry | Count |
| --- | ---: |
| Arena ends at or before the guard | 136 |
| Arena end exceeds the guard start | 4984 |
| Arena ends exactly at the stack base | 17 |

These are domain counts, not measured production probabilities or QEMU proof.
For stack draw `s` and mmap draw `m`, the arena overlaps the guard whenever
`s + m > 15`. Template/TCB reservations need additional disjoint space.

## Concrete guard counterexample

Use stack draw 16 and mmap draw 0. Offsets from USER_REGION_START are:

- stack base: `0x1e0000`;
- intended guard: `[0x1df000, 0x1e0000)`;
- mmap slot 15: `[0x1d0000, 0x1e0000)` for a full 64 KiB allocation.

`task::mmap_current` chooses a free mapping slot and delegates to
`userspace::map_anonymous`. That function bounds slot/size/alignment but does
not exclude the stack guard. `paging::map_user_range_in_impl` refuses an
already-mapped page; an intentionally unmapped guard does not satisfy that
check. In this counterexample the allocation ends before the first mapped
stack page. Source analysis therefore indicates that allocating the full last
slot can remove the intended guard, rather than merely returning AlreadyMapped.
A dedicated forced-draw CPU test must establish the actual runtime failure
before implementing and accepting the correction.

QEMU feature builds deliberately use zero draws. Their existing success is
valid for the deterministic bounded contract, not for randomized geometry.
The new TLS loader rejects a reservation crossing the mmap bound, safely
failing load; that does not repair the inherited geometry for ordinary images.

## Required correction gate

After the current TLS prerequisite passes full acceptance, build a validated
joint address-space plan, preserving fixed ET_EXEC placement, nonzero PIE
relocation, the established sixteen-slot/64 KiB capacities, W^X and unmapped
guards. Keep all regions disjoint: main ELF, full anonymous arena, immutable
TLS template, mutable TLS/TCB, guard and stack, plus the bounded library windows.
Audit maximum image/reservation limits rather than silently widening them.

Do not disable ASLR, replace real draws with zero, reduce slot capacity or
accept random load failure as the fix. Exercise every supported draw in pure
host geometry tests, then actual nonzero/extreme-draw CPU execution, full arena
mapping and guard-fault behavior on 1/2/4 CPUs. Preserve old zero-draw behavior
and all inherited gates. Review overflow, malformed image/range inputs, fork,
thread allocation and teardown. Commit source and acceptance separately.

Until that evidence exists, randomized placement and full Stage 11/1–19
production readiness remain open regardless of the bounded TLS outcome.
