# WovenUpdate

WovenUpdate provides two bounded entry points:

- `--check` (or `--all`) inventories `/updates` without changing state.
- `--service` publishes the privileged `woven.update` IPC service.

The service accepts the existing fixed-capacity WUDI ABI. It rejects malformed
requests before invoking the kernel, never transports image bytes over IPC,
and forwards valid lifecycle operations to `libwoven::update_transaction`.
Responses are always encoded as fixed-size WUDI responses, including malformed
requests when a response can be delivered to the sender.

The IPC receive ABI is nonblocking, so an empty queue yields to the scheduler;
the daemon does not timer-poll or spin. The kernel remains authoritative for
signature verification, capability checks, durable A/B state, slot writes,
activation, rollback, and recovery. `--gui` remains the existing interactive
preflight mode.
