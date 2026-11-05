# WovenSettings

WovenSettings is a bounded system-settings application. It reports the current
process identity, UTC clock, and compositor display policy through the ordinary
Ring-3 `libwoven` ABI and provides an interactive theme preview: `l` selects
Light, `d` selects Dark, and `c` selects High Contrast. These changes are
session-local previews; persistent settings and policy changes remain owned by
a future service boundary.

Modes are `--all`, `--identity`, `--clock`, `--display`, and `--help`.
