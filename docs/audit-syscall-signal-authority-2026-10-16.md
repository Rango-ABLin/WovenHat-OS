# Audit: signal authority and handler validation (2026-10-16)

**Baseline:** `claude/production-audit-20261015` (PR #80) atop
`phase2/userspace-platform` `494ae66`.
**Result:** two defects fixed in source. Host and lint validation pass.
QEMU evidence comes from the PR's CI lanes, not a local acceptance run.

This continues the syscall-boundary audit started in
[the user-pointer bounds audit](audit-syscall-user-pointer-bounds-2026-10-15.md).

## Defect 1: any process could kill any other process

`kill` (syscall 26) went straight to `task::kill_process`. That function
checked only `sig <= 31` before signalling every matching PID or process
group.

Every Ring-3 process runs as uid 1000 with `CapabilitySet::userspace()`, so
any application could `kill(pid, 9)` any other one. That included system
services and the WovenShell desktop session.

POSIX permits a signal only when the sender's uid matches or it holds
privilege (Linux: `CAP_KILL`). With a single uid, a uid rule would protect
nothing here, so the fix uses the authority this OS already has:

| Sender | May signal |
|---|---|
| Kernel (self-tests, kernel shell after its own `TaskControl` check) | any process (unchanged) |
| Ring-3 process | itself and its own children |
| Ring-3 process holding `TaskControl` | any process |

- **Where it applies.** `sys_kill` now calls `kill_process_from_user`.
  Kernel callers keep `kill_process`.
- **Process groups.** Each selected member is checked separately; refused
  members are skipped. If processes matched but none could be signalled,
  the call fails (`PermissionDenied`, `-1`), like Linux `EPERM`. Signal 0,
  the existence probe, is checked the same way, as POSIX requires.
- **Lock order.** `current_has` takes and releases the scheduler lock
  before the process table is locked, so lock order is unchanged.
- **Compatibility.** No Ring-3 code in the tree sends `kill`, other than
  an unused libc stub and the user shell's `kill` command, which no gate
  drives. `libwoven` and the apps never call it.

## Defect 2: a signal handler address could halt the kernel

`sigaction` (syscall 34) stored any value above 1 as a handler.
`prepare_signal_delivery` then wrote it into the saved user RIP, and the
syscall returned with `iretq`.

For a non-canonical handler, `iretq` raises #GP **in ring 0**, at the
`iretq` instruction itself. `general_protection_fault_handler` recovers
only user-mode faults, so it called `halt()`. Two syscalls were enough:

```
sigaction(10, 0x8000_0000_0000_0000)
kill(getpid(), 10)      ->  #GP at iretq in ring 0  ->  halt()
```

The fix has two parts:

- **At registration.** `sigaction_current` accepts a handler only if it is
  0 (default), 1 (ignore), or inside the caller's own executable image
  (`contains_executable`). This is the rule `create_user_thread` already
  applies to thread entry points.
- **At delivery.** `prepare_signal_delivery` refuses any handler outside
  the canonical user half (`user_address::check_user_range`), as defence in
  depth.

## Checked and found sound

- **Thread entry points.** `create_user_thread` validates `entry` with
  `contains_executable`.
- **`ThreadSetTls`.** It stores a value that only `ThreadGetTls` reads
  back. The FS base loaded at context switch comes from the loader's TLS
  mapping, not from this value.
- **`unwrap`s in `syscall.rs`.** All are on fixed-size slices and cannot
  fail.
- **Allocations sized by user arguments.**
  - `graphics_write`: the surface size bounds it (1920×1200×4 bytes).
  - `audio_play`: limited to 64 KiB.
  - Both use a heap that grows on demand.

## Allocation failure in syscall paths (fixed 2026-10-16)

Kernel allocations sized by user arguments used infallible `alloc::vec!`.
If physical memory was exhausted, the allocation-error handler halted the
kernel instead of failing the syscall (Linux returns `ENOMEM`).

They now use fallible allocation. On exhaustion the call returns
`SYSCALL_ERROR`, audited as a failure where the path already audits:

- `heap::try_filled(value, len)` (`try_reserve_exact`) for:
  - the exec/spawn image buffers (`sys_exec`, `sys_exec_command`,
    `sys_spawn_command`);
  - the `sys_audio_play` PCM copy;
  - the `sys_graphics_write` pixel copy.
- `display::Surface::try_new` for `user_graphics::create`, which
  returns `SystemFull`.

Sizes were already bounded: surfaces are capped at 1 Mpixel per process
and 4 Mpixels system-wide; audio at 64 KiB. The heap returns null when
growth fails, so `try_reserve_exact` reports the failure instead of
reaching `alloc_error_handler`. Kernel-internal allocations (boot, self
tests, storage recovery) are unchanged.

## Evidence

- **Ring-3 probes.** Added to the integrated user program that every boot
  runs; any wrong result exits non-zero and fails the boot:
  - The parent `kill(child, 0)` must return 0.
  - The forked child's `kill(getppid(), 0)` must return `-1`. Otherwise
    the child exits 1, and the parent requires 42.
  - `sigaction` with a non-canonical handler and with a kernel-half
    handler must return `-1`. Ignore and default must still work, and
    return the previous disposition.
- **Program size.** The program is 944 bytes (limit 4096).
- **Lint.** Kernel Clippy `-D warnings` is clean for the default,
  `qemu-test`, `stage11-2-test`, `stage11-5-test`, `stage20-15-test` and
  `system-apps` features. Host `--all-targets` Clippy is clean.
- **Host tests.** All 154 pass.
- **Not run.** No local QEMU (acceptance hold; no QEMU in this checkout).
  The negative control, running the probes against the old kernel, has not
  been run either.
