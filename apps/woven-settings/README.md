# WovenSettings

WovenSettings is a bounded system-settings application. It reports the current
process identity, UTC clock, and compositor display policy through the ordinary
Ring-3 `libwoven` ABI and provides an interactive theme preview: `l` selects
Light, `d` selects Dark, and `c` selects High Contrast. High-contrast policy is
persisted through the bounded installed-settings record and, when a writable
system volume is mounted, its durable source copy. Light and dark remain visual
previews because the shell currently has a dark/light default plus the
persisted high-contrast policy.

Modes are `--all`, `--identity`, `--clock`, `--display`, and `--help`.
