# WovenUpdate

WovenUpdate is a bounded, read-only update preflight client. It inventories
the `/updates` directory and reports candidate package names without installing
or activating them. Signature verification, A/B activation, rollback, and
power-loss recovery belong to the privileged update service and are never
simulated by this Ring-3 client.

Modes are `--check` and `--help`.
