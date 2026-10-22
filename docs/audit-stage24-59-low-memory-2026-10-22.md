# Stage 24.59 low-memory regression audit — 2026-10-22

The 256 MiB Change-PIN regression fixture initially failed before the desktop
because the bootstrap heap mapped 2,473,984 bytes while the compositor needed
a 4,096,000-byte allocation. The production 4 GiB floor remains unchanged;
this fixture is retained as a low-memory regression profile.

The heap bootstrap now reserves its bounded 8 MiB ceiling whenever the
available frame count can afford that reservation, while retaining the
proportional 256 KiB–8 MiB policy below that threshold. This keeps the heavy
desktop fixture viable without making the heap unbounded or changing runtime
growth limits.

Verification:

- `cargo test -p wovenhat-kernel`: passed.
- `rustfmt --edition 2021 --check kernel/src/heap.rs`: passed.
- Stage 24.59 QEMU, 1 CPU, 256 MiB fixture: passed with exit 33.
- The gate passed kernel/user-space boot, Ada sign-in, Quick Settings,
  current/new/confirmation PIN entry, durable rotation, old-PIN rejection,
  and new-PIN unlock.

Evidence is retained in `target/woven-shell-change-pin-1cpu`.

The shell's new 3D renderer remains opt-in until its full pixel-exact
acceptance matrix is re-derived; the accepted Change-PIN path runs with the
flat default.
