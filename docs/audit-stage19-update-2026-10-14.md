# Stage 19 WovenUpdate audit — 2026-10-14

WovenUpdate is a separate static Ring-3 application. It performs a bounded,
read-only inventory of `/updates` and explicitly refuses to model installation
or rollback. No package is activated, deleted, or trusted by the client;
signature verification, A/B selection, power-loss recovery, and rollback are
reserved for the privileged update service.

The two host parser tests and warning-denying freestanding Clippy passed. The
signed external-application harness passed on 1, 2, and 4 CPUs with exit 33;
serial/QEMU evidence is preserved in
`audit-artifacts/woven-update-{1,2,4}cpu`.

The nine-application catalog and production-image integration consume the
package. The update-service transaction and persistent media policy remain
separate roadmap requirements.
