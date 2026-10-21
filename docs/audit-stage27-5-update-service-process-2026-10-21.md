# Stage 27.5 WovenUpdate service-process audit — 2026-10-21

## Scope

This slice connects the signed `woven-update` system application to the
production `system-apps` boot path. The kernel starts it as a system service
with `Ipc`, `FileRead`, `FileWrite`, and `StorageIo`, and waits for the
`woven.update` publication before completing service startup.

The daemon itself publishes `woven.update`, receives bounded WUDI requests,
rejects malformed requests before privileged dispatch, forwards validated
operations to syscall 239 through `libwoven`, and returns bounded responses.

## Evidence

| Check | Result | Retained evidence |
|---|---|---|
| `apps/woven-update` host tests | 2 passed | command output on 2026-10-21 |
| `apps/woven-update` freestanding Clippy, `-D warnings` | passed | command output on 2026-10-21 |
| production system-app shell gate, 1 CPU | passed | `target/shell-regression-1cpu/serial.log` |
| production system-app shell gate, 2 CPUs | passed | `target/shell-regression-2cpu/serial.log` |
| production system-app shell gate, 4 CPUs | passed | `target/shell-regression-4cpu/serial.log` |

Each serial log contains both:

```text
[SERVICES] woven-package published as pid 1
[SERVICES] woven-update published as pid 2
```

The QEMU gate also reports `Normal release shell + PS/2 IOAPIC + smptest:
PASS` for each CPU count.

## Limits and next prerequisites

This audit proves boot-time publication and service ownership only. It does
not prove a client/server transaction exchange, production-sized block-device
slot storage, bootloader handoff, power-loss recovery, release-key
provisioning, or physical USB qualification. Those remain release gates.
