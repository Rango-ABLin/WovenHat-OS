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

## Bootstrap increment: minimal CRT entry

`ports/musl/src/crt_start.S` consumes WovenHat's established initial stack and
calls `__wovenhat_start_c`, which invokes `main(argc, argv)` and exits through
upstream musl `_Exit`. QEMU proves this entry path at 1/2/4 CPUs. It is not
musl `__libc_start_main`: auxiliary vectors, environment setup, TLS, init/fini
hooks, locale, and constructors remain explicit prerequisites.

## Bootstrap increment: close error path

The bootstrap now compiles upstream musl `close.c`; its weak no-AIO fallback
is retained because async I/O cancellation is not implemented. The fixture
proves denied `close(-1)` returns `-1` with `errno == EIO` at 1/2/4 CPUs.
Successful descriptor close and standard-descriptor setup remain kernel ABI
work, not implied by this error-path proof.

## Bootstrap increment: console write

The static fixture proves upstream musl `write(1, ...)` returns its full byte
count through WovenHat's existing capability-gated console path on 1/2/4 CPUs.
It does not create a file-table entry for stdout or relax the kernel policy
that descriptors below 3 cannot be closed.

## Bootstrap increment: restricted open policy

`ports/musl/src/open.c` exposes only a bounded `O_RDONLY` translation to the
current WovenHat path-plus-length syscall. Null, empty and overlong paths, and
every flag other than `O_RDONLY`, fail locally with `ENOSYS`; flags and mode
are never discarded. The executable fixture verifies `O_WRONLY` rejection at
1/2/4 CPUs. A stable readable VFS fixture and a kernel flags/mode ABI are
required before successful open/create semantics can be certified.

## Bootstrap increment: read-only open/close lifecycle

The fixture opens the boot-provided `/etc/motd` with `O_RDONLY`, requires a
non-standard descriptor, and closes it through upstream musl `close.c`.
The entire lifecycle passed at 1/2/4 CPUs. This certifies only the existing
read-only path-plus-length kernel contract; create, write modes, offsets and
descriptor duplication remain separate work.

## Bootstrap increment: read-only descriptor read

The same `/etc/motd` lifecycle now reads all 24 boot-provided bytes through
upstream musl `read.c` and validates stable content before close. The full
open/read/close path passed at 1/2/4 CPUs. File offsets, partial reads,
nonblocking behavior and writable descriptors remain unimplemented.
