# Stage 25.7 wake-trampoline staging audit (2026-10-21)

## Accepted scope

The existing AP real-mode-to-long-mode transition image is now reusable for
the future S3 wake path. Staging clones the current kernel root, builds a
private low-memory identity map for the first 1 GiB, patches the reserved wake
page with the transition image, and supplies a dedicated aligned wake stack and
entry point. The allocated page-table frames are owned by a private
`WakeTrampoline` token and are released on abort or self-test teardown.

Suspend preparation now stages this image after driver quiescence and rolls
back both the image resources and driver state if staging fails. The guarded
`stage25-7-test` QEMU gate installs and tears down the image without entering
S3 or modifying firmware state.

## Verification

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage25-7-test -- -D warnings` passed.
- Default production-kernel Clippy with `-D warnings` passed.
- Host all-targets Clippy with `-D warnings` passed.
- `cargo test --all` passed.
- `scripts/test-stage25-7-wake-trampoline.py` passed on 1, 2, and 4 CPUs.
- `git diff --check` passed with only the repository's CRLF conversion warning.

Per-CPU evidence is retained under `audit-artifacts/stage25.7-*`.

## Explicit limits

This stage does not write a FACS waking vector, save CPU/register state,
coordinate or restart secondary CPUs, enter ACPI S3, handle firmware wake, or
claim physical suspend/resume qualification. The wake entry currently
reinitializes the descriptor/interrupt foundation and parks until the later
resume-state boundary is implemented.
