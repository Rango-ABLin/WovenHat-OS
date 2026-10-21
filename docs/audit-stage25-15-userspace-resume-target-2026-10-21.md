# Stage 25.15 userspace resume target audit — 2026-10-21

## Scope

Suspend action `2` now validates the complete syscall frame supplied by the
existing interrupt entry against the current task's kernel stack. The frame is
retained in the committed suspend record. The wake continuation restores all
owned resources, writes a successful return value, and transfers through the
existing syscall epilogue to the interrupted userspace caller.

## Verification

The required build, Clippy, host-test, and 1/2/4-CPU acceptance results will be
recorded here after the Stage 25.15 tree passes. The focused QEMU test does not
issue S3; it verifies record ownership and invalid-pointer rejection only.

## Limits

This stage does not prove that a physical machine enters S3, wakes through the
FACS vector, or resumes every real driver. Physical firmware and USB-boot
qualification remain separate release gates.
