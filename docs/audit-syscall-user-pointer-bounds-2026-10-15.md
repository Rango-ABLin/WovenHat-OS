# Audit: syscall user-pointer bounds (2026-10-15)

**Baseline:** `phase2/userspace-platform` at `494ae66`.
**Result:** the defect is fixed in source, and host and lint validation pass.
The QEMU matrix has **not** been run (see Limits). This is not full stage
acceptance.

## Defect

Any Ring-3 process could panic the kernel with one syscall.

`paging::copy_from_current_user` and `paging::copy_to_current_user` walked
the page tables for a user-supplied address through `translated_chunk`,
which called `x86_64::VirtAddr::new(address)`. In `x86_64` 0.15.5 that
function **panics** when bits 48–63 are not a sign extension of bit 47.
Nothing on the syscall path bounded the pointer first.

Reproduction path (by reading the code; not executed in QEMU):

```
sys_write(1, 0x0001_0000_0000_0000, 1)
  -> paging::copy_from_current_user
  -> copy_current_user
  -> translated_chunk
  -> VirtAddr::new(non-canonical)
  -> panic!  (whole kernel stops)
```

All 54 callers of the two copy routines were exposed, across file, path,
IPC, async, network and graphics syscalls. No capability is needed:
`write` to fd 1 is open to every process.

A second, lesser gap: kernel-half addresses were refused only because the
**leaf** page entry lacked `USER_ACCESSIBLE`. Hardware requires the U bit at
every level, so the leaf-only check is weaker than what the CPU enforces.

## Fix

- New `kernel/src/user_address.rs`: `check_user_range(start, length)`
  accepts a range only if it lies entirely below `USER_ADDRESS_LIMIT`
  (`0x0000_8000_0000_0000`), without wrapping. This is the equivalent of
  Linux `access_ok()`. An empty range is accepted, which preserves the
  previous behaviour for zero-length copies.
- Both copy routines call it before taking `PAGING` or walking tables.
  Overflow maps to `AddressOverflow`; out-of-range maps to
  `PermissionDenied`. No caller matches on the variant (all map to
  `SYSCALL_ERROR`), so the syscall ABI is unchanged.
- `translated_chunk` now uses `VirtAddr::try_new` as defence in depth, so
  a future caller that skips the bound gets an error instead of a panic.

Unchanged and reviewed:
- COW breaking before writes, file-fault handling, and dropping `PAGING`
  around fault resolution.
- Lock order: the check takes no lock.
- Kernel-internal loader writers (`write_user_bytes`,
  `zero_user_range_in`), whose addresses come from ELF segments already
  bounded by `elf::USER_ADDRESS_LIMIT`.
- `munmap_current`, which only matches mappings the kernel recorded.

## Evidence

All run on Linux, nightly-2026-07-10:

- **New host test.** `cargo test --test user_address` passes 6 tests:
  in-range, empty, non-canonical, kernel-half, crossing the limit, and
  wrapping.
- **Full host suite.** `cargo test` passes all 154 tests, with no
  failures.
- **Kernel Clippy.** `cargo clippy -p wovenhat-kernel --target
  x86_64-unknown-none -- -D warnings` is clean for the default
  (production), `qemu-test`, `stage11-5-test`, `stage20-15-test` and
  `system-apps` features. The last two were linted with placeholder app
  images and trust keys, because their real values come from the
  harness.
- **Host Clippy.** `cargo clippy --all-targets -- -D warnings` is clean.
- **Baseline.** Before the change, the same baseline was clean on kernel
  Clippy, host Clippy and host tests.

## Limits and next prerequisites

- **No QEMU evidence.** The user-confirmed hold in
  `docs/agent-coordination.md` reserves the next acceptance slot for
  Codex, and this cloud checkout has no QEMU. Before merging to `master`,
  run on 1/2/4 CPUs:
  - the release chain (`scripts/test-release.py`);
  - the Stage 7–11 user-memory, IPC and async gates.
- **No Ring-3 negative test yet.** The next gate should add one: a
  process passes non-canonical, kernel-half and limit-crossing pointers to
  `write`, `read`, `open` and an async submission, and expects
  `SYSCALL_ERROR` for each, with the kernel still alive.
