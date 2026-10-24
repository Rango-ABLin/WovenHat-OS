# Stage 19 WovenMonitor audit — 2026-10-14

WovenMonitor is a separate static Ring-3 application. It has a bounded mode
parser and read-only reports for UTC hardware-clock time, compositor geometry
when `DisplayIo` is available, and a capped `/mnt` directory-entry count.
Unknown or overlong modes fail closed; no write, display-scale mutation, or
new capability was introduced.

Host parser tests and warning-denying freestanding Clippy passed. The signed
external-application harness passed on 1/2/4 CPUs with exit 33 and preserved
serial/QEMU evidence in `audit-artifacts/woven-monitor-{1,2,4}cpu`.

The application is intentionally limited to ABI data available to an ordinary
Ring-3 process. Kernel-wide CPU/memory counters and privileged device telemetry
remain outside the current ABI and are not fabricated by this app.
