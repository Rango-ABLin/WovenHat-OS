# Stage 23.7 audit — advisory WovenAI security monitor

Date: 2026-10-20

## Scope

This stage adds fixed-size advisory explanations over the existing security
audit event ABI. WovenGuard remains the only enforcement authority; the
monitor cannot grant, revoke, or mutate capability policy.

## Acceptance

The dedicated gate was run serially with the QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 7 --cpus 1` — PASS,
  exit 33, `[S23.7] WovenAI security monitor: PASSED`; evidence:
  `audit-artifacts/stage23.7-1cpu-1792525067904963600`
- `python scripts/test-stage23-1-ai-broker.py --stage 7 --cpus 2` — PASS,
  exit 33, `[S23.7] WovenAI security monitor: PASSED`; evidence:
  `audit-artifacts/stage23.7-2cpu-1792525085640869300`
- `python scripts/test-stage23-1-ai-broker.py --stage 7 --cpus 4` — PASS,
  exit 33, `[S23.7] WovenAI security monitor: PASSED`; evidence:
  `audit-artifacts/stage23.7-4cpu-1792525103539579600`

## Static and host checks

- `cargo test -p libwoven --lib` — 80 passed.
- Stage 23.7 freestanding kernel Clippy with `-D warnings` — pass.
- Host all-targets Clippy — pass.
- Python syntax validation and `git diff --check` — pass.

## Limits and next prerequisites

Anomaly detection, monitor UI, notification transport, response automation,
and real-world policy tuning remain open. The monitor is advisory only and
does not qualify production security operations or physical hardware.
