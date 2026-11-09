# WovenEditor

WovenEditor is a bounded Ring-3 text editor and note writer. Its desktop
window provides a real UTF-8 editor field, keyboard input/backspace, Save
button and Enter-to-save action; the GUI saves the current note to the bounded
`/mnt/woven-note.txt` path through the explicit filesystem-write ABI. The
kernel still enforces the caller's `FileRead`/`FileWrite` capabilities and VFS
policy.

Modes are `--view PATH`, `--write PATH TEXT`, and `--help`.
