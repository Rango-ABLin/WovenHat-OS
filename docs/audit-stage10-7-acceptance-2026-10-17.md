# Stage 10.7 acceptance evidence — 2026-10-17

The user-provided transcript from `RUN-STAGE10.7.ps1` ends with:

`=== STAGE 10.7 ACCEPTANCE: PASS ===`

The transcript reports host TCP regressions (33 tests), host lint, Cargo
build/Clippy/tests, preservation through Stages 7.6–10.6, and dedicated
async-TCP boots on 1, 2, and 4 CPUs. Each CPU count passed the host TCP
exchange: connect, two transmissions, receive, and FIN/EOF, with QEMU exit
33 and retained serial logs under the project-local `target` tree.

This records the supplied run only. It does not replace the full release
validation chain, physical USB/xHCI evidence, or AX201 hardware qualification.
