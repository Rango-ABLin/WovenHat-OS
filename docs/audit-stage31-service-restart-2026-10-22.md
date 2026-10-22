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
