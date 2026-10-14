# WovenEditor

WovenEditor is a bounded Ring-3 text preview and note writer. `--view` reads
at most 2048 bytes from a file; `--write PATH TEXT` writes a bounded note
through the explicit filesystem-write ABI and flushes it. The kernel still
enforces the caller's `FileRead`/`FileWrite` capabilities and VFS policy.

Modes are `--view PATH`, `--write PATH TEXT`, and `--help`.
