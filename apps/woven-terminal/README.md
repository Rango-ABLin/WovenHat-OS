# WovenTerminal

WovenTerminal is a separate Ring-3 console application. It provides bounded
`help`, `ls PATH`, `cat PATH`, `calc EXPR`, and `exit` commands through the
existing `libwoven` filesystem and process ABI. Directory output is capped at
128 entries and file output at 16 × 512-byte chunks per command. The parser
does not allocate and rejects ambiguous or overlong commands. Pass
`--interactive` to run a bounded stdin loop with the same line limit; EOF and
input errors terminate cleanly.

Build with `cargo build --release` from this directory; the local Cargo
configuration selects the pinned `x86_64-unknown-none` target and static ELF
link contract. Signed packaging and the 1/2/4-CPU Ring-3 acceptance matrix
are run by `scripts/test-terminal-qemu.py`.
