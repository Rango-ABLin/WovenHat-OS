# Stage 31 service-restart slice — 2026-10-22

## Scope

The signed `woven-update` service acceptance now exercises a complete
restart boundary: terminate and reap the first daemon, verify endpoint
cleanup, reload the signed image, spawn a fresh process, verify republished
ownership, rediscover a new capability handle, service a query, and tear the
second daemon down cleanly.

## Evidence

- `python scripts/test-stage27-5-update-service.py --cpus 1` — passed, exit
  33.
- Serial marker: `[S27.5] IPC daemon restart/republication: PASSED`.
- Evidence: `audit-artifacts/stage27.5-1cpu/serial.log`.
- `$env:WOVENHAT_UPDATE_IMAGE=target/woven-update.wapp; cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage27-5-test -- -D warnings` — passed.

## Limits

This is signed userspace-service restart qualification, not a general
supervisor, driver restart, automatic retry policy, sudden power-loss
recovery, or native hardware certification. Those require separate owner,
restart, and hardware evidence.

## 2026-10-23 bounded production supervisor increment

`kernel/src/service_supervisor.rs` now registers the production package,
update, and installer service PID generations. The kernel event loop detects
an exited child, reaps it through the owning parent, applies a bounded
100/200/400-tick retry backoff, reloads the verified image, and publishes a
replacement generation. Each service has at most three restart attempts;
stale results cannot replace a newer PID and budget exhaustion disables the
entry with a serial audit record.

Evidence:

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features
  system-apps -- -D warnings` passed using the existing packaged artifacts
  and trusted test key.
- The Windows host has no local OVMF image, so a live crash/restart QEMU
  execution was not claimed in this increment; CI must execute the packaged
  system-app acceptance before this slice is runtime-accepted.

The explicit PCI driver rebind slice remains separate and still lacks an
automatic driver-crash supervisor.
