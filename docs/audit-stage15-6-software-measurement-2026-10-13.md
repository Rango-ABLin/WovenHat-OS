# Stage 15.6 measured boot — software measurement accepted (2026-10-13)

The kernel now performs a bounded SHA-256 measurement of the bootloader-loaded
kernel ELF range after the physical-memory mapping is established and before
driver/userspace activation. The measured range is limited to 64 MiB and is
read only through the bootloader-provided physical address and length. A
known-answer SHA-256 self-test and a serial measurement prefix are included.

Warning-denying freestanding Clippy passed. The runtime gate passed with exit
33 on all required CPU counts:

- `audit-artifacts/stage15.6-1cpu-1791902683781855300`
- `audit-artifacts/stage15.6-2cpu-1791902842148564400`
- `audit-artifacts/stage15.6-4cpu-1791902986405576100`

This is not TPM-backed measured boot. The current bootloader API does not pass
TPM event-log access or UEFI Secure Boot state to the kernel. Firmware PCR
extension, event-log publication, signed kernel/initrd enforcement, and
hardware qualification remain open Stage 15.5/15.6 requirements.
