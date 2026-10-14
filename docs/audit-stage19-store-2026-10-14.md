# Stage 19 WovenStore audit — 2026-10-14

WovenStore is a separate static Ring-3 application. It enumerates the bounded
`/share/applications` catalog and reports installed descriptors without
installing, removing, or trusting packages. Package acquisition, signature
policy, dependency resolution, and transactional installation remain owned by
a future privileged package service.

The two host parser tests and warning-denying freestanding Clippy passed. The
signed external-application harness passed on 1, 2, and 4 CPUs with exit 33;
serial/QEMU evidence is preserved in
`audit-artifacts/woven-store-{1,2,4}cpu`.

The ten-application catalog and production-image integration consume the
package. Full store UI and package-service workflows remain separate platform
work.
