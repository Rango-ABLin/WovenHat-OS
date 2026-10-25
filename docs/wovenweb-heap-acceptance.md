# WovenWeb experimental reclaiming heap

This adds an opt-in native Rust heap prerequisite for WovenWeb. It does not
complete native Ladybird, the C++ runtime, TLS, Tor, VPN or OS installation.
The default SDK allocator remains the legacy allocator; only the browser probe
enables `libwoven/reclaiming-allocator`.

The bitmap core tracks ownership outside allocation payloads, reuses freed spans,
combines adjacent regions, respects alignment, rolls back failed growth and
quarantines uncertain unmaps. The process-local atomic guard serializes heap
metadata and mapping calls without disabling interrupts. A raw-thread entry
trampoline establishes the C ABI stack frame. Event wait masks are checked
before the syscall, including reserved bits and duplicate masks.

## Validation and current acceptance boundary

177 SDK unit tests and three compile-fail tests passed. Repository host tests
passed. Eight Python network harness tests cover packet parsing and Cargo image
metadata extraction. Native fixture evidence is indexed in the adjacent JSON.
The fixture checks 800 thread allocations, alignment through 64 KiB, cross-region
realloc, zeroing, 512 reuse cycles, mapping-slot exhaustion and recovery, a 256 KiB
allocation and ordinary Rust Vec integration. Child teardown checks exact frames,
mappings and process counts. Guest threads currently share the creator CPU;
1/2/4-CPU configurations do not prove parallel guest allocator execution. The
host backend separately exercises four concurrent threads.

Windows Application Control blocked the generated root build script during
workspace Clippy (error 4551). Remaining warning-denying feature/probe checks,
fresh-image rebuild and network/legacy-GUI regressions are incomplete. No policy
was disabled or bypassed. This is a draft implementation, not stage acceptance.
The reused guest image predates a documentation-only allocator comment edit;
its original source hashes are preserved separately from current checkout hashes.

## Runtime limits

The kernel still allows sixteen 64 KiB anonymous mappings. Other mapping users
consume the same roughly 1 MiB window. The heap keeps one idle region cached and
returns additional empty regions. It cannot provide a browser-sized address space.
Raw mmap/munmap, thread stack creation and fork/exec/teardown require a kernel VM
reservation/lease layer for general concurrent use; the heap lock coordinates
only its own calls. The controlled fixture stages unrelated VM operations.
Quarantine does not repair partial kernel VM transactions. The spin guard offers
no priority inheritance, asynchronous cancellation or fork recovery. VM callbacks
must never allocate or reenter. Production capabilities and VM ceilings are unchanged.

Next prerequisites are transactional VM ownership, larger address-space bounds,
a cross-built libc/C++ ABI fixture and LibCore adaptation before the engine port.

Reproduce with `python scripts/test-wovenweb-memory-qemu.py --cpus 1`, followed
by `--cpus 2 --image PATH` and `--cpus 4 --image PATH` using the emitted image.
Build metadata supplies the image path without executing the host launcher.
