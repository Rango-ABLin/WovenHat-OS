# Native C++ ABI fixture

This independent Ring-3 ELF links header-free C++23 object files into a Rust
entry/runtime adapter. Clang targets `x86_64-unknown-none-elf`; there are no
Windows, Linux, libc or C++ standard-library headers/libraries in the guest.

Set `WOVENWEB_CLANG` to a Clang executable and ensure its host DLL dependencies
are available in that process's PATH. Run `cargo build --release --locked` from
this directory. The compiler command is in build.rs; no C++ LTO is used, so the
separate dispatch translation unit exercises a virtual call. The pinned host
toolchain packages/checksums are in toolchain.json. Do not treat those host
libraries as guest runtime libraries.

From the repository root run:

```text
python scripts/test-wovenweb-cxx-qemu.py --cpus 1 --qemu PATH_TO_QEMU
python scripts/test-wovenweb-cxx-qemu.py --cpus 2 --image EMITTED_IMAGE --qemu PATH_TO_QEMU
python scripts/test-wovenweb-cxx-qemu.py --cpus 4 --image EMITTED_IMAGE --qemu PATH_TO_QEMU
```

The harness expects firmware at QEMU's sibling share/edk2-x86_64-code.fd and
records source/image hashes, compiler identity for fresh builds, serial output
and result JSON under audit-artifacts/wovenweb-cxx-*. It grants Console only;
no NIC, host filesystem mount or production catalog entry is added.

Coverage: eight-integer SysV calls including stack arguments, aggregate layout
and return, zero/static data, guarded local static initialization, stack RAII,
512 class-specific aligned allocations, virtual dispatch and deletion, destructor
counts, native heap cleanup, child exit and exact kernel resource baselines. One
retained global C++ initializer and its registered destructor run through a
bounded private `__cxa_atexit` fixture adapter; hosted Linux guest qualification
passed on 1/2/4 CPUs. See [the global-lifecycle evidence](../../docs/wovenweb-cxx-global-lifecycle-evidence.json).

Limits: this is a small ABI fixture. Class allocation/deallocation knows the
exact Widget layout; global delete deliberately fails if unexpectedly reached.
Guard/static-object shims remain private to the single-thread, no-exceptions
part of this fixture. No general
new/delete ABI, libc++, exception unwinding, RTTI, multithreaded static guards,
dynamic TLS/TLS destructors, floating-point ABI, general global
constructor/destructor arrays or Ladybird engine
is qualified here. Production VM transactions remain a separate prerequisite.

Static compiler thread-local storage is qualified by the hosted 92328129 matrix
on 1/2/4 CPUs: initialized/zero words, 64-byte alignment, two sibling immutable
templates and distinct addresses, 64 yields each, and main FS/value restoration
after joins. Guard/object checks still run single-threaded after those joins.
The compile uses static local-exec TLS and PT_TLS; there is no dynamic TLS or
thread-local destructor support. This is unrelated to HTTPS transport security.
All existing ABI/object/heap checks and exact kernel cleanup remain. Kernel/probe
lints and complete Rust/PR CodeQL pass (no new alerts, zero PR annotations).
Explicit --firmware/--firmware-vars support Linux with private VARS copies.
See ../../docs/wovenweb-cxx-static-tls.md and its evidence JSON.
