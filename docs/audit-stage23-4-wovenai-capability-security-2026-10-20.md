# Stage 23.4 audit — WovenAI capability security

Date: 2026-10-20

## Scope

This stage turns the AI authority rule into an executable WovenGuard acceptance
gate. Local AI receives only `Ipc + Ai`; cloud provider authority is explicitly
`Ipc + Secrets + NetworkIo`; broker authority remains `Ipc`. File masks,
device masks, domain ceilings, and capability membership are checked through
the same policy functions used at launch.

## Acceptance

The dedicated gate was run serially with the QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 4 --cpus 1` — PASS,
  exit 33, `[S23.4] WovenAI capability security: PASSED`; evidence:
  `audit-artifacts/stage23.4-1cpu-1792524134603006000`
- `python scripts/test-stage23-1-ai-broker.py --stage 4 --cpus 2` — PASS,
  exit 33, `[S23.4] WovenAI capability security: PASSED`; evidence:
  `audit-artifacts/stage23.4-2cpu-1792524151189661900`
- `python scripts/test-stage23-1-ai-broker.py --stage 4 --cpus 4` — PASS,
  exit 33, `[S23.4] WovenAI capability security: PASSED`; evidence:
  `audit-artifacts/stage23.4-4cpu-1792524165773461400`

The gate verifies that local AI has no file/device authority, cloud authority
allows network only when explicitly declared, and system services cannot hold
the sensitive AI/secret/camera/microphone capabilities.

## Static checks and limits

- Stage 23.4 freestanding kernel Clippy with `-D warnings` — pass.
- Prior Stage 23.1/23.2/23.3 QEMU gates remain green.
- The policy gate does not qualify HTTPS/TLS, real cloud providers, semantic
  search, assistant actions, or physical USB/network hardware.
