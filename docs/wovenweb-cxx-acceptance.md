# WovenWeb native C++ ABI fixture

A Clang-built, header-free C++23 fixture now runs inside WovenHat as a separate
Ring-3 ELF linked with a Rust native runtime adapter. It does not depend on a
host or guest libc/C++ standard library. This accepts a small controlled ABI
fixture, not the browser runtime stage or a Ladybird engine build.

## Verified behavior

The same final boot image passed on 1/2/4 CPUs with QEMU exit 33. The guest checks
eight-integer C++ to Rust SysV calls including stack arguments, a two-u64 aggregate
layout/return, zero-initialized/static data, one-time local-static initialization,
a stack object's RAII destructor, and 512 aligned dynamic virtual objects.
Separate C++ translation units without C++ LTO preserve the virtual call boundary.
Class-specific new/delete forwards exact layouts to the experimental native Rust
heap. Construction/destruction counters agree (513 each including the stack
object), and no live heap allocations remain. Child exit/reaping restores exact
mapping, physical-frame and process baselines.

Probe release Clippy and kernel feature Clippy for wovenweb-cxx-test,
wovenweb-memory-test, wovenweb-net-test and stage19-2-test pass with -D warnings.
The fresh root host-suite rerun passed 150 tests before Windows Application
Control refused the TLS-layout executable (4551), so this run is incomplete.
The previous full 211-test run is historical evidence. The root workspace Clippy
executable remains policy-blocked too; no policy was bypassed. Ordinary compilation
and the guest matrix work.
The initial test-launch failure used a QEMU found in MSYS2 without its expected
firmware path; its evidence is retained. Final runs explicitly use the qualified
Program Files QEMU/firmware pair, without changing assertions or deadlines.

The ELF is x86-64 System V ET_EXEC, with two load segments and entry
0x400000000000. The kernel gate grants only Console and adds no production
catalog entry, syscall, capability or VM ceiling. Source, compiler, image and
serial hashes are in [the evidence index](wovenweb-cxx-evidence.json).

## Reproduction and toolchain

See [the probe README](../tools/wovenweb-cxx-probe/README.md) and its
[toolchain manifest](../tools/wovenweb-cxx-probe/toolchain.json). The workspace-only
host compiler was obtained from the official MSYS2 Clang 23.1.3 packages and all
three archive hashes matched the package pages. Compiler, clang-libs and llvm-libs
archives were extracted into work/toolchains rather than installed globally.
The compiler uses existing MSYS2 UCRT64 DLL dependencies, identified separately
from guest code. Host toolchain binaries are not committed or embedded in WovenHat.

Official package metadata:
- https://packages.msys2.org/packages/mingw-w64-ucrt-x86_64-clang
- https://packages.msys2.org/packages/mingw-w64-ucrt-x86_64-clang-libs
- https://packages.msys2.org/packages/mingw-w64-ucrt-x86_64-llvm-libs

## Remaining prerequisites

The guard shims are private to this single-thread, no-exceptions fixture. They do
not qualify recursive initialization, cancellation, fork recovery or concurrent
mixed-width atomic guards. Class allocation knows the exact Widget layout;
unexpected global delete exits with failure. No general new/delete, libc++, libc,
exception unwinding, RTTI, TLS, floating-point ABI, global initializer/finalizer
arrays or compiler runtime beyond this fixture is qualified. The bounded heap
still has the existing sixteen 64 KiB mapping slots. Production VM ownership,
capacity and pager/thread/fork/exec/teardown integration remain open before a
larger runtime or engine port. See [VM ownership](wovenweb-vm-transactions.md).
