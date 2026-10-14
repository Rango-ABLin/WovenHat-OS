# Writing WovenHat applications

This describes how to build a program that runs on WovenHat in Ring 3, loaded
from the filesystem like any other executable.

Until Stage 16.1 this was not possible. Every userspace program was a
`global_asm!` block inside `kernel/src/userspace.rs` — there are 32 of them —
wrapped in a synthetic ELF by `build_stub_elf` at runtime. Writing a program
meant editing the kernel. `apps/hello` is the first application built by the
ordinary Rust toolchain, and it is the template for the rest.

## The shape of an application

An application is a crate outside the kernel workspace. `apps/hello` has four
files worth understanding.

**`Cargo.toml`** declares its own `[workspace]` table. Applications are listed
in the root manifest's `exclude`, not its `members`, because a host-targeted
workspace build must not try to compile a freestanding Ring-3 image.

**`src/main.rs`** is `#![no_std]` and `#![no_main]`, exports `_start`, and ends
by calling `libwoven::exit`. It must also provide a `#[panic_handler]`; a
userspace panic should terminate the process rather than spin.

**`link.x`** places the image and declares its segments.

**`.cargo/config.toml`** carries the target and the three link settings the
kernel's loader requires.

## What the loader accepts

`kernel/src/elf.rs` is strict on purpose, and each rule has a consequence for
how an application must be built.

| Loader rule | What it means for you |
| --- | --- |
| `e_type` must be `2` (`ET_EXEC`) | A static, non-PIE executable. `relocation-model=static` and `--no-pie`. |
| `PT_DYNAMIC`, `PT_INTERP`, `PT_TLS`, `PT_GNU_RELRO` are rejected | No dynamic linking, no interpreter, no thread-locals, and `-z norelro`. |
| A segment may not be both writable and executable | Code and data belong in separate page-aligned segments. `link.x` declares explicit `PHDRS` for this. |
| Entry must be at or above `USER_REGION_START` | Link at `0x4000_0000_0000`. |
| `e_ehsize` must be 64, machine must be x86-64 | Standard ELF64; nothing to do. |

Three build settings follow from the address, and each exists for a concrete
reason rather than by convention:

- **`code-model=large`.** `USER_REGION_START` is `0x4000_0000_0000`, far outside
  the ±2 GB that the default small code model assumes. Without this, every
  reference to a static fails to link with
  `relocation R_X86_64_32S out of range`.
- **`build-std = ["core"]`.** The precompiled `core` is built with the small
  code model, so linking it against a large-model crate fails with
  `Code Model: IDs have conflicting values`. Rebuilding `core` from source makes
  both agree; `rust-src` is pinned in `rust-toolchain.toml`.
- **`--script=link.x`.** Supplies the load address and the segment split.

## The address space you are given

| Region | Address |
| --- | --- |
| Image base | `0x4000_0000_0000` |
| `mmap` arena | `0x4000_0000_0e_0000` |
| Stack | `0x4000_0000_1f_0000` |

An image must stay below the `mmap` arena, which leaves roughly 900 KiB for
code and data. The kernel enters at `_start` with a mapped stack already in
place, so an ordinary function prologue is safe — no entry trampoline is needed.

Address-space layout randomisation shifts the image, stack and `mmap` base on
normal boots. It is disabled under `qemu-test` so acceptance runs stay
deterministic.

## Using the kernel from an application

Link `libwoven` and call it rather than issuing `int 0x80` by hand.

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let _ = libwoven::print("hello\n");

    let mut digits = [0u8; 20];
    let pid = libwoven::format_u64(libwoven::current_pid().0, &mut digits);
    let _ = libwoven::write(libwoven::syscall::STDOUT, pid);

    libwoven::exit(0)
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
```

`libwoven` is layered. `libwoven::syscall` holds one `unsafe` wrapper per kernel
entry point, returning the kernel's value unchanged so a caller can distinguish
`ERROR` from `WOULD_BLOCK`. Above it sit safe functions — `print`, `write`,
`read`, `spawn`, `wait`, `sleep`, `ticks`, `current_pid`, `exit` — which return
`Result<_, libwoven::Error>` wherever the unsafety can be discharged.

There is no allocator and no `core::fmt` worth its size in this environment, so
`format_u64` covers the common case of reporting a number.

The kernel implements 98 syscalls and `libwoven` currently exposes the process,
I/O, filesystem, time and environment core. The rest — sockets, DNS, the async
completion API, `mmap`, pipes, signals — are reachable through
`libwoven::syscall::invoke` today and should grow typed wrappers as they are
used.

## Building and running

```powershell
cd apps\hello
cargo build --release
```

The image lands at
`apps/hello/target/x86_64-unknown-none/release/wovenhat-hello`.

To run it under the acceptance gate:

```powershell
python .\scripts\test-application-qemu.py --cpus 1
```

That builds the application, passes its path to the kernel build through
`WOVENHAT_APP_IMAGE`, wraps it in a signed WAPP package, boots, and requires
the Stage 16.1 markers. The kernel verifies the package, installs it into the
filesystem, reads it back, and loads **the bytes it read from the filesystem**
rather than the embedded constant, so the gate proves signature enforcement and
the filesystem path rather than assuming either.

## Adding a new application

1. Copy `apps/hello` to `apps/<name>`.
2. Add `apps/<name>` to the root manifest's `workspace.exclude`.
3. Keep the `[workspace]` table, `link.x` and `.cargo/config.toml` as they are.

## Known limits

- **One application per acceptance build.** `WOVENHAT_APP_IMAGE` names a single
  source ELF and the build produces one signed WAPP package. A multi-application
  package/catalog format is still open work.
- **No FAT32 round trip yet.** The gate installs into the VFS at `/bin`, which
  is where `exec` resolves bare names. Persisting an application onto a FAT32
  volume and executing it across a reboot is a separate increment.
- **No shared libraries.** The loader rejects `PT_DYNAMIC` by design, so every
  application is statically linked.
- **No thread-locals.** `PT_TLS` is rejected, so `#[thread_local]` cannot be
  used.
- **No stack size negotiation.** The kernel chooses the stack; an application
  cannot request a larger one.
