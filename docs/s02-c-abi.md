# S02 freestanding C ABI contract

S02 now has a reproducible build boundary in `scripts/build-c-abi-fixture.py`.
It compiles `tools/runtime-fixtures/c-abi.c` with a target-qualified clang,
links it with `tools/runtime-fixtures/link.x`, and validates the result as a
static little-endian x86-64 ELF whose entry is in the WovenHat user region.

The fixture deliberately uses no libc, startup object, allocator, TLS,
`errno`, pthreads, locale, math runtime, destructors, or shared libraries. It
only exercises the documented `_start` entry and exit syscall ABI. A successful
build is not execution acceptance; the resulting ELF must still be loaded by
the real WovenHat loader and exit successfully under QEMU before S02 closes.

Run:

```text
python scripts/build-c-abi-fixture.py --clang <target-qualified-clang>
```

When no compiler exists, the script writes a `NOT_AVAILABLE` manifest and
returns exit code 2. It never converts missing toolchain support into a pass.
