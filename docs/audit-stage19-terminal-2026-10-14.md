# Stage 19 WovenTerminal application audit — 2026-10-14

WovenTerminal is the second roadmap Stage 19 system application delivered as
a separate crate. It uses `libwoven` and the existing signed WAPP/ELF path;
it does not add kernel privileges or an alternate ABI.

## Application boundary

- The command parser is allocation-free and rejects commands over 128 bytes,
  ambiguous prefixes, and missing arguments.
- `ls` caps output at 128 entries. `cat` caps each invocation at 16 chunks of
  512 bytes, closes its descriptor on every path, and reports open/read
  failures instead of panicking.
- `calc` delegates checked arithmetic to the bounded WovenCalculator core.
- `--interactive` reads stdin one byte at a time through the existing file
  descriptor ABI, buffers at most 128 bytes per line, and terminates cleanly
  on EOF or read failure.
- The application is a static ET_EXEC image with W^X-compatible segments and
  is signed by the existing WAPP packager.

## Evidence

- Two host command-parser tests passed.
- Host library Clippy and freestanding binary Clippy passed with `-D warnings`.
- Signed WAPP packaging and Ring-3 execution passed on 1/2/4 CPUs (exit 33):
  `audit-artifacts/woven-terminal-1cpu`,
  `audit-artifacts/woven-terminal-2cpu`, and
  `audit-artifacts/woven-terminal-4cpu`.

The QEMU acceptance proves package verification, filesystem round-trip, ELF
loading, and clean zero-exit execution. The acceptance invokes the
deterministic argv path; interactive hardware/console input and boot-catalog
installation remain separate integration evidence.
