# WovenTerminal

WovenTerminal is a separate Ring-3 console application. The desktop Terminal
window accepts commands interactively and executes the same bounded command
path as the CLI; command output is written to the QEMU serial/monitor console
so it can be inspected while testing. It provides bounded
`help`, `ls PATH`, `cat PATH`, `calc EXPR`, and `exit` commands through the
existing `libwoven` filesystem and process ABI. Other commands are executed
from `/bin` through a bounded shell path supporting single/double quoting,
backslash escapes, `$NAME`/`${NAME}` expansion, up to four pipeline stages,
`<`, `>`, `>>`, `2>`, `2>>`, trailing `&`, `jobs`, and `fg %N`. Every stage is
forked with inherited descriptors and process-group assignment; foreground
jobs temporarily own the terminal foreground group and background jobs are
reaped from a four-entry job table. Directory output is capped at 128 entries
and file output at 16 × 512-byte chunks per command. The parser does not
allocate and rejects overlong commands. Pass
`--interactive` to run a bounded stdin loop with the same line limit; EOF and
input errors terminate cleanly. In interactive QEMU, run
`python scripts/run-interactive-qemu.py`, sign in, and open the Terminal icon.

Build with `cargo build --release` from this directory; the local Cargo
configuration selects the pinned `x86_64-unknown-none` target and static ELF
link contract. Signed packaging and the 1/2/4-CPU Ring-3 acceptance matrix
are run by `scripts/test-terminal-qemu.py`.
