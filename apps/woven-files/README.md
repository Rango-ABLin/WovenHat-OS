# WovenFiles

WovenFiles is a read-only Ring-3 file browser increment. It lists a bounded
directory with `--path PATH` (default `/mnt`) and previews a bounded 2048-byte
file with `--view PATH`. It uses only `libwoven` file-read operations; no write
or privilege bypass is present. The signed external-application harness is
used for validation.
