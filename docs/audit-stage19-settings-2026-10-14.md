# Stage 19 WovenSettings audit — 2026-10-14

WovenSettings is a separate static Ring-3 application. It provides bounded,
read-only identity, UTC clock, and compositor display-policy reports through
the ordinary `libwoven` ABI. Invalid and overlong modes fail closed; the app
does not mutate settings or request privileged device authority.

The two host parser tests and warning-denying freestanding Clippy passed. The
signed external-application harness passed on 1, 2, and 4 CPUs with exit 33;
serial/QEMU evidence is preserved in
`audit-artifacts/woven-settings-{1,2,4}cpu`.

The five-application Stage 19 catalog gate passed on 1/2/4 CPUs, and the
4-CPU production `system-apps` boot installed all five packages before shell
startup. Persistent settings storage and a policy service remain outside this
bounded read-only application increment.
