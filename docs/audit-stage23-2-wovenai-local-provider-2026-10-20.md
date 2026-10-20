# Stage 23.2 audit — bounded WovenAI local provider

Date: 2026-10-20

## Scope

This stage adds the first local WovenAI provider. It is deliberately bounded:
fixed integer weights, a fixed feature vocabulary, no allocation, bounded WAI1
UTF-8 input, an explicit `MODEL_ABI`, and no filesystem, network, secret,
device, or accelerator access. It is not a general neural-model runtime.

## Acceptance

The dedicated gate was run serially with the existing QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 2 --cpus 1` — PASS,
  exit 33, `[S23.2] WovenAI local provider: PASSED`; evidence:
  `audit-artifacts/stage23.2-1cpu-1792523271142588200`
- `python scripts/test-stage23-1-ai-broker.py --stage 2 --cpus 2` — PASS,
  exit 33, `[S23.2] WovenAI local provider: PASSED`; evidence:
  `audit-artifacts/stage23.2-2cpu-1792523298416240400`
- `python scripts/test-stage23-1-ai-broker.py --stage 2 --cpus 4` — PASS,
  exit 33, `[S23.2] WovenAI local provider: PASSED`; evidence:
  `audit-artifacts/stage23.2-4cpu-1792523326958053700`

The probe verifies the declared `Ipc + Ai` capability boundary, service
publication, broker discovery, local-provider descriptor/model ABI, expected
classification result, and broker teardown. The denied probe must fail with
the capability-denied exit path.

## Static and host checks

- `cargo test -p libwoven --lib` — 71 passed.
- Kernel freestanding Clippy with `stage23-1-test` and `stage23-2-test` — pass.
- Host all-targets Clippy — pass.
- `apps/woven-ai` freestanding release Clippy — pass.
- Python syntax validation and Stage 21 feature lint — required release checks.

## Limits and next prerequisites

This acceptance does not qualify arbitrary model files, tensor runtimes,
hardware acceleration, cloud providers, semantic search, assistant actions,
or physical USB media. Those remain later stages and require their own
capability, resource, recovery, and 1/2/4-CPU acceptance evidence.
