# WovenMonitor

WovenMonitor is a bounded, read-only Ring-3 diagnostic application. It
reports UTC hardware-clock time, compositor geometry when the caller has
`DisplayIo`, and a capped count of `/mnt` directory entries. It never writes
files, changes display scale, or requests additional capabilities.

Build with `cargo build --release`; signed packaging and 1/2/4-CPU acceptance
are performed by the repository release harness.
