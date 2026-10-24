# WovenSettings

WovenSettings is a bounded read-only system-settings application. It reports
the current process identity, UTC clock, and compositor display policy through
the ordinary Ring-3 `libwoven` ABI. It does not mutate settings or claim
privileged device authority; persistent settings and policy changes remain
owned by a future service boundary.

Modes are `--all`, `--identity`, `--clock`, `--display`, and `--help`.
