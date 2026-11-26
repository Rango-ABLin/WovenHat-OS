# WovenFiles

WovenFiles is a bounded Ring-3 file browser. It lists a readable directory with
`--path PATH` (default `/mnt`) and previews a bounded 2048-byte file with
`--view PATH`. Bounded mutations (`--mkdir PATH`, `--rm PATH`, `--rename OLD NEW`,
`--write PATH TEXT`) are confined to `/mnt/...` or `/tmp/...` and still require
the kernel's `FileWrite` capability and mounted-path policy.

The GUI shows a selectable `/mnt` row list, a path field and New/Delete/Rename
actions that reuse the same capability-checked wrappers. The kernel remains
authoritative: no privilege bypass, no new syscall and no new capability is
introduced. Signed 1/2/4-CPU and desktop visual harnesses validate the app.
