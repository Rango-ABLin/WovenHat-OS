# Physical readiness preflight — 2026-10-22

Command:

```text
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\test-physical-tpm-readiness.ps1
```

The read-only preflight completed successfully and wrote its machine-specific
report under `audit-artifacts/physical-tpm-readiness-20261022-153529/`.

Observed boundary:

- Physical TPM visible: no
- Physical TPM ready: no
- QEMU TPM backend available: no
- Signed provisioning record present: no
- USB targets found: 0
- Raw disk opened, disk state changed, media written, TPM provisioned: none

This is a safety and scheduling result, not a failed software test. Native
TPM, USB installation, and physical hardware qualification remain unqualified
until suitable hardware or an approved emulation backend is available.
