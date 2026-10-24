# Stage 28 installer plan validator audit — 2026-10-22

This slice adds `scripts/validate-installer-plan.py`, a pure validator for a
caller-supplied immutable inventory snapshot and explicit installation
settings. It requires an exact path/serial identity match, rejects missing or
ambiguous matches, and protects system/boot, removable, read-only, backup,
external, and USB-labelled disks.

The validator bounds disk and partition sizes, requires sector-aligned
partitions, reserves one MiB, limits partition count, and validates the
filesystem, username, supported timezone/keyboard, and DHCP/static network
fields. Its canonical JSON includes a stable schema, normalized settings,
summary, and explicit `disk_io: none` / `installation: not-implemented`
operation markers.

Focused tests cover valid plans, protected and ambiguous targets, overflow and
capacity failures, invalid settings, and deterministic output. The validator
does not open device paths, invoke discovery, partition, format, install, or
install a bootloader. It does not claim installation support.
