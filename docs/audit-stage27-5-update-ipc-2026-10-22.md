# Stage 27.5 end-to-end WovenUpdate IPC audit — 2026-10-22

## Scope

This gate exercises the actual signed `woven-update` userspace image rather
than only calling the kernel transaction functions. The acceptance client:

1. builds and signs the daemon image;
2. loads it through verified ELF/package validation;
3. grants only `Ipc`, `FileRead`, `FileWrite`, and `StorageIo`;
4. discovers `woven.update`;
5. sends a WUDI Query and verifies the active-slot response;
6. sends a truncated frame and verifies the fixed bad-request response; and
7. terminates, waits for, and verifies service cleanup.

## Results

| Check | Result | Evidence |
|---|---|---|
| WovenUpdate host tests | 2 passed | command output, 2026-10-22 |
| WovenUpdate freestanding Clippy | passed | `-D warnings` |
| Stage 27.5 kernel Clippy | passed | `x86_64-unknown-none`, `-D warnings` |
| Workspace all-target Clippy | passed | `-D warnings` |
| QEMU 1 CPU | passed, exit 33 | `audit-artifacts/stage27.5-1cpu` |
| QEMU 2 CPUs | passed, exit 33 | `audit-artifacts/stage27.5-2cpu` |
| QEMU 4 CPUs | passed, exit 33 | `audit-artifacts/stage27.5-4cpu` |

The first failed run is retained and was diagnosed: the daemon had allocated a
4096-byte receive buffer while the kernel IPC syscall accepts at most 256
bytes. The daemon now uses a 256-byte transport buffer, while the WUDI codec
continues to reject malformed or oversized frames. No assertions were
weakened.

## Remaining limits

This accepts the daemon IPC exchange only. It does not provide arbitrary-size
block-device update slots, bootloader target/attempt persistence, power-loss
injection, WovenRecovery, installer integration, or physical USB/hardware
qualification.
