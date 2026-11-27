# WovenWeb global allocator repair - 2026-11-27

## Scope and root cause

CI run `46028455258` at commit `308a31f2` failed all three WovenWeb native CPU
matrix entries at the memory fixture. The serial log showed every explicit
runtime-heap check passing, then `[WOVENWEB-HEAP] exit=Some(68)` and
`[WOVENWEB-HEAP] native memory platform: FAILED`. Exit 68 is the fixture's
`Vec::<u32>::try_reserve_exact(20000)` failure.

The fixture calls `libwoven::heap::runtime_allocator()` directly for its
explicit checks, but standard Rust collections use the crate's
`#[global_allocator]`. A merge of master into the WovenWeb foundation branch
replaced the opt-in `RuntimeHeap<NativeVm>` global allocator with the legacy
fixed 64 KiB `Arena` while leaving `runtime_allocator()` pointing at the
reclaiming heap. The 80,000-byte `Vec` reservation therefore failed against the
arena even though the reclaiming heap can span adjacent 64 KiB regions.

Repair:

- Gate the legacy `Arena` and its `#[global_allocator]` static on
  `not(feature = "reclaiming-allocator")` in `libwoven/src/lib.rs`.
- Apply `#[global_allocator]` to `USER_HEAP` in
  `libwoven/src/heap_allocator.rs` for native `reclaiming-allocator` builds.
- Add a host contract test that preserves both gates and the fixture's full
  20,000-element reservation. No fixture assertion or allocation size was
  reduced, and no QEMU marker was relaxed.

The unrelated Alt+Tab edits already present in `libwoven/src/lib.rs` were not
staged by this repair.

## Local evidence

| Gate | Result |
| --- | --- |
| `cargo clippy -p libwoven --target x86_64-unknown-none --locked -- -D warnings` | 0 |
| `cargo clippy -p libwoven --target x86_64-unknown-none --features reclaiming-allocator --locked -- -D warnings` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 |
| `cargo fmt --all -- --check` | 0 |
| `cargo build --release --locked` in `tools/wovenweb-net-probe` with its `.cargo/config.toml` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 |
| `cargo fmt --all -- --check` | 0 |
| `cargo test --workspace --locked` | 0 |
| `python -X utf8=0 -m unittest discover -s tests -p 'test_*.py'` | 292 tests OK |

The `[RNG] boot entropy: unavailable (fails closed)` line in the CI log is the
documented honest TCG result, not the cause of exit 68.

## QEMU evidence

Serialized with the P0 owner through `docs/agent-coordination.md`; no other
acceptance chain was run during these gates.

| CPUs | Result | Evidence |
| --- | --- | --- |
| 1 | PASS, exit 0, required marker present | `audit-artifacts/wovenweb-memory-1795795495823627300-1cpu`, `C:/Users/banda/AppData/Local/Temp/opencode/wovenweb-heap-qemu-1cpu.log` |
| 2 | PASS, exit 0, required marker present | `audit-artifacts/wovenweb-memory-1795795592623673200-2cpu`, `C:/Users/banda/AppData/Local/Temp/opencode/wovenweb-heap-qemu-2cpu.log` |
| 4 | PASS, exit 0, required marker present | `audit-artifacts/wovenweb-memory-1795795617053328000-4cpu`, `C:/Users/banda/AppData/Local/Temp/opencode/wovenweb-heap-qemu-4cpu.log` |

All three runs produced `[WOVENWEB-HEAP] standard Rust Vec integration: PASSED`
and `[WOVENWEB-HEAP] native memory platform: PASSED` with balanced mappings,
frames and processes.

## Limits

No native hardware category is claimed. No USB, firmware, TPM or boot entry was
touched. Full CI green is not claimed until the replacement WovenWeb, kernel
and Windows acceptance runs on the repaired commit complete.
