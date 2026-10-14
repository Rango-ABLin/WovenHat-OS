# Stage 19 WovenFiles application audit — 2026-10-14

WovenFiles is the third roadmap Stage 19 system-application increment. It is a
separate static Ring-3 image using `libwoven` read-only filesystem calls.

The default invocation lists `/mnt`; `--path PATH` lists another authorized
directory and `--view PATH` emits a bounded 2048-byte preview. Directory
enumeration is capped at 128 entries, file output is copied through a fixed
buffer, descriptors are closed on every read path, and invalid UTF-8 or open/
read failures return a nonzero status. No write or privilege authority is
introduced.

Freestanding Clippy passed with `-D warnings`. Signed WAPP packaging and the
existing verified external-application Ring-3 acceptance passed on 1/2/4 CPUs:

- `audit-artifacts/woven-files-1cpu`
- `audit-artifacts/woven-files-2cpu`
- `audit-artifacts/woven-files-4cpu`

The app is read-only and argument-driven for deterministic acceptance. A full
interactive GUI, persistent boot catalog, and write/mutation workflow remain
later integration work.
