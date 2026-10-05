# Stage 13.10T — outbound BLE key-size authorization gate

Date: 2026-10-05

The Stage 13.10 runtime acceptance now requires
`[S13.10T] Bluetooth LE notification minimum key size: PASSED`. A dedicated
self-test rejects an authenticated 12-byte session when the protected
subscription requires 16 bytes, then accepts a 16-byte session and verifies
the encoded notification.

The exact tip passed build, warning-denying kernel Clippy, 11 host tests, and
the Stage 13.10 QEMU gate on 1/2/4 CPUs. The QEMU harness exited with its
expected success code 33 and retained serial evidence under
`audit-artifacts/stage13.10-*`. Physical BLE interoperability, SMP/LTK
provisioning and hardware qualification remain open.
