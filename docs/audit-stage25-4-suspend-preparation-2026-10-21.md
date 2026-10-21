# Stage 25.4 suspend-preparation audit (2026-10-21)

## Accepted scope

Stage 25.4 adds a bounded, reversible preparation boundary for a future ACPI
S3 transition. `driver::suspend_all` snapshots bound drivers without holding
the driver-table lock across hardware callbacks, suspends them in registration
order, and rolls back already-suspended drivers in reverse order if any hook
refuses. `driver::resume_all` also resumes in reverse order and reports an
aggregate result.

`power::prepare_suspend` requires the discovered ACPI PM1a control block, an
`_S3` sleep state, and ACPI mode before creating the preparation token.
`power::abort_suspend` consumes that token and restores driver state. The
preparation token is intentionally private and cannot be confused with a
completed sleep transition.

## Verification

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --all` passed, including the full libwoven and integration-test suite.
- Signed system-apps QEMU acceptance passed on 1, 2, and 4 CPUs via `scripts/test-shell-qemu.py --system-apps --cpus N`.
- `git diff --check` passed with only the repository's existing CRLF conversion warning.

The QEMU serial evidence is retained in `target/shell-regression-1cpu/`,
`target/shell-regression-2cpu/`, and `target/shell-regression-4cpu/`.

## Explicit limits

This stage does not write ACPI `SLP_EN`, enter S3, install a wake vector,
coordinate CPU resume, select wake sources, or add a suspend syscall. It also
does not provide device-specific save/restore beyond the existing driver
hooks, battery/thermal/lid reporting, or physical suspend/resume qualification.
