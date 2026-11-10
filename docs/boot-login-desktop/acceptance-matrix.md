# Acceptance matrix

Status: IN PROGRESS. Phases A-E are COMPLETE; Phase F is the next software
gate. Phase B physical firmware/trust qualification remains explicitly
deferred to the final hardware pass.

| Gate | Required evidence | Current status |
|---|---|---|
| A: baseline/architecture | source inventory, existing tests, startup/security trace | COMPLETE |
| B: UEFI handoff | positive 1/2/4-CPU boots, missing/corrupt kernel failures | COMPLETE (software/QEMU; physical firmware, PXE/NIC and production Secure Boot remain hardware/trust work) |
| C: init/readiness | essential dependencies and bounded failure recovery | COMPLETE (bounded WovenInit DAG, ownership/liveness readiness, deadline and all-process rollback at 1/2/4 CPUs) |
| D: splash | framebuffer captures and ownership transition | COMPLETE (terminal-owned branded splash, truthful timer scope, exclusive compositor handoff, and visual 1/2/4-CPU evidence) |
| E: authentication | entropy, protected storage, wrong credentials, rate limits, first run | COMPLETE (kernel-owned plaintext verification, protected verifier storage, secure-entropy failure closure, installer provisioning, missing-account refusal, and 1/2/4-CPU evidence) |
| F: login | graphical/input/accessibility evidence through real authority | NOT STARTED |
| G: session | owner/identity isolation, replay rejection, lock/logout cleanup | NOT STARTED |
| H: desktop | authenticated launch and real taskbar/apps/windows/input | NOT STARTED |
| I: integration | repeated 1/2/4-CPU login/logout/reboot/crash/persistence runs | NOT STARTED |
| J: release candidate | reproducible image, interactive launcher, CI, limitations | NOT STARTED |

Existing functionality will be credited only after source and evidence review;
NOT STARTED here means the new phase gate has not been executed in this unit.

Negative tests must reject desktop authorization without authentication,
forged/expired sessions, changed user IDs, unauthorized credential reads,
recovery bypass, overlong input, malformed messages and privilege escalation.
Graphics require screenshots or framebuffer inspection, not serial text alone.
Use original assertions and budgets; preserve every failure log.
