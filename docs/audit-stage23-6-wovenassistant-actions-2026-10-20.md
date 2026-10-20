# Stage 23.6 audit — WovenAssistant permissioned actions

Date: 2026-10-20

## Scope

This stage adds a fixed-capacity action planner. Actions have bounded validated
targets, map to explicit Woven permission authorities, and require a separate
confirmation for side effects. The planner does not execute platform actions;
execution remains a permission-broker and capability-policy responsibility.

## Acceptance

The dedicated gate was run serially with the QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 6 --cpus 1` — PASS,
  exit 33, `[S23.6] WovenAssistant actions: PASSED`; evidence:
  `audit-artifacts/stage23.6-1cpu-1792524840375472300`
- `python scripts/test-stage23-1-ai-broker.py --stage 6 --cpus 2` — PASS,
  exit 33, `[S23.6] WovenAssistant actions: PASSED`; evidence:
  `audit-artifacts/stage23.6-2cpu-1792524861370942600`
- `python scripts/test-stage23-1-ai-broker.py --stage 6 --cpus 4` — PASS,
  exit 33, `[S23.6] WovenAssistant actions: PASSED`; evidence:
  `audit-artifacts/stage23.6-4cpu-1792524875749738800`

## Static and host checks

- `cargo test -p libwoven --lib` — 78 passed.
- Stage 23.6 freestanding kernel Clippy with `-D warnings` — pass.
- Host all-targets Clippy — pass.
- Python syntax validation and `git diff --check` — pass.

## Limits and next prerequisites

Permission-broker integration, user confirmation UX, audit-visible action
records, cancellation/teardown, and actual application/file/network executors
remain open. The acceptance does not claim complete assistant functionality or
physical hardware qualification.
