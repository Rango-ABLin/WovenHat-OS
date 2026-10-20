# Stage 23.3 audit — WovenAI provider-plugin boundary

Date: 2026-10-20

## Scope

This stage adds a separate Ring-3 provider process and broker delegation path.
The provider advertises a bounded ABI, owns its WovenVault credential handle,
and returns only fixed-size WAI1 responses. The broker never receives the
credential bytes. The acceptance provider is an offline deterministic fixture;
it is not an arbitrary cloud API or HTTPS/TLS implementation.

## Acceptance

The dedicated gate was run serially with the QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 3 --cpus 1` — PASS,
  exit 33, `[S23.3] WovenAI cloud provider: PASSED`; evidence:
  `audit-artifacts/stage23.3-1cpu-1792523642289709300`
- `python scripts/test-stage23-1-ai-broker.py --stage 3 --cpus 2` — PASS,
  exit 33, `[S23.3] WovenAI cloud provider: PASSED`; evidence:
  `audit-artifacts/stage23.3-2cpu-1792523675601682100`
- `python scripts/test-stage23-1-ai-broker.py --stage 3 --cpus 4` — PASS,
  exit 33, `[S23.3] WovenAI cloud provider: PASSED`; evidence:
  `audit-artifacts/stage23.3-4cpu-1792523708187643800`

The gate starts the provider with `Ipc + Secrets + NetworkIo`, starts the
broker with `Ipc` only, verifies both service publications, checks unauthorized
client denial, confirms an authorized probe receives the provider-marked
result, and reaps both processes.

## Static and host checks

- `cargo test -p libwoven --lib` — 73 passed.
- Stage 23.3 freestanding kernel Clippy with `-D warnings` — pass.
- Host all-targets Clippy — pass.
- `apps/woven-ai` freestanding release Clippy — pass.
- Python syntax, `git diff --check`, and all Stage 21 feature lints — pass.
- Stage 23.1 and 23.2 regression gates — pass on 1/2/4 CPUs.

## Limits and next prerequisites

Real cloud endpoint configuration, TLS/certificate verification, credential
provisioning and rotation, retry/backoff, quota/billing controls, network
availability policy, and physical network/USB qualification remain open.
