# Stage 1–5 transaction boundary audit — 2026-10-14

`persist_paths_atomic` now rejects more than 255 entries with
`PersistError::TooLarge` before checking mount state or touching a block
device. The kernel storage self-test exercises a 256-entry request and checks
that exact result. This keeps the bounded WDJ2 limit explicit and fail-closed;
it does not claim arbitrary-size or full-volume rollback.
