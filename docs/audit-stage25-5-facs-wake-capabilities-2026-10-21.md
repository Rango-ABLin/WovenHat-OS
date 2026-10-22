# Stage 25.5 FACS wake-capability audit (2026-10-21)

## Accepted scope

ACPI discovery now follows the FADT firmware-control pointer, prefers the
validated 64-bit X_FACS address when present, and retains FACS metadata only
when the containing firmware region, `FACS` signature, and minimum length are
valid. The retained record includes the FACS address/length/version and both
firmware waking-vector fields.

Syscall 238 and `libwoven::suspend_resume_capabilities` expose only bounded
capability flags and the FACS version to a caller with `PowerControl`:
advertised S3, validated FACS, nonzero 32-bit vector, and nonzero 64-bit
vector. No firmware address is copied to userspace, and the query does not
write FACS or PM1 control state.

The ACPI self-test covers valid FACS metadata plus short-length and signature
rejection. Invalid FACS data fails closed to “not available” rather than
making an untrusted wake structure usable.

## Verification

- Default production-kernel Clippy with `-D warnings` passed.
- `system-apps` production-kernel Clippy with `-D warnings` passed.
- Host all-targets Clippy with `-D warnings` passed.
- `cargo test --all` passed: 95 libwoven tests and all integration suites.
- Signed system-apps QEMU acceptance passed on 1, 2, and 4 CPUs.
- `git diff --check` passed with only the repository's CRLF conversion warning.

Serial evidence remains in `target/shell-regression-1cpu/serial.log`,
`target/shell-regression-2cpu/serial.log`, and
`target/shell-regression-4cpu/serial.log`.

## Explicit limits

This stage does not allocate an OS wake trampoline, write the FACS waking
vector, save/restore CPU state, enter S3, handle firmware wake, coordinate
secondary CPUs, or expose a suspend action. It also does not qualify a real
machine's firmware or power-loss behavior.
