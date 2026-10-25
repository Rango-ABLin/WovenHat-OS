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
counts, native heap cleanup, child exit and exact kernel resource baselines.

Limits: this is a small ABI fixture. Class allocation/deallocation knows the
exact Widget layout; global delete deliberately fails if unexpectedly reached.
Guard shims are private to a single-thread no-exceptions fixture. No general
new/delete ABI, libc++, exception unwinding, RTTI, multithreaded static guards,
TLS, floating-point ABI, global constructor/destructor arrays or Ladybird engine
is qualified here. Production VM transactions remain a separate prerequisite.
