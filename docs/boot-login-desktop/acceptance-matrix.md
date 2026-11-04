# Acceptance matrix

Status: IN PROGRESS. Phase A is COMPLETE at runtime revision a980619e;
later phases are not accepted.

| Gate | Required evidence | Current status |
|---|---|---|
| A: baseline/architecture | source inventory, existing tests, startup/security trace | COMPLETE |
| B: UEFI handoff | positive 1/2/4-CPU boots, missing/corrupt kernel failures | NOT STARTED |
| C: init/readiness | essential dependencies and bounded failure recovery | NOT STARTED |
| D: splash | framebuffer captures and ownership transition | NOT STARTED |
| E: authentication | entropy, protected storage, wrong credentials, rate limits, first run | NOT STARTED |
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
