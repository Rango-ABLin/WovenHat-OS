# WovenHat POSIX port program

WovenHat will use musl `v1.2.6` as the MIT-licensed libc base. This is a new
program after the accepted S02 freestanding C ABI boundary; it does not change
the meaning of S02 acceptance.

The port has two versioned profiles:

- `POSIX.1-2017`: baseline interfaces and behavior.
- `POSIX.1-2024`: the 2017 profile plus separately tested 2024 additions.

Each exported interface must state which profile(s) it supports. A 2024-only
interface must not alter 2017 behavior.

## Required ordering

1. Replace musl's Linux syscall transport and syscall-number assumptions with
   a WovenHat-owned `int 0x80` adapter, retaining checked pointer/capability
   validation in the kernel.
2. Provide C runtime entry, TLS, errno, allocator, process and thread hooks;
   prove static libc startup and teardown with independently compiled C.
3. Implement and qualify file, directory, time, signal, pipe/FIFO, terminal,
   socket and process-group semantics against their profile requirements.
4. Bring up musl dynamic-loader/shared-library support only after static libc,
   relocation, lifecycle and TLS ownership tests are green.
5. Run profile-specific conformance tests, adversarial ABI tests and QEMU
   1/2/4-CPU matrices. Native-hardware and external certification remain
   separate evidence.

No profile is considered conformant merely because headers compile or a Linux
syscall number is accepted. Unsupported calls must fail predictably, never be
silently mapped to a different WovenHat authority.

## Bootstrap increment: upstream I/O objects

`scripts/build-musl-wovenhat-bootstrap.py` generates musl's `alltypes.h` from
the pinned sources, compiles upstream `read.c`, `write.c`, and `syscall_ret.c`,
and links a static C ELF with the WovenHat transport bridge. The executable
proves that a denied `write()` returns `-1` and records the current transport
failure as `errno == EIO` before exiting. Its errno storage is intentionally
single-threaded: TLS, C startup, allocator, and thread cancellation semantics
have not yet been implemented. The bridge rejects unmapped calls with ENOSYS;
it does not discard extra arguments or route them through another authority.
