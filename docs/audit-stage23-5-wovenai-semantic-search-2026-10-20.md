# Stage 23.5 audit — bounded semantic search core

Date: 2026-10-20

## Scope

This stage adds the first bounded search index for applications, files,
settings, messages, documents, and commands. It validates UTF-8 titles,
normalizes bounded ASCII terms, scores query-term coverage, and returns stable
ranked hits from fixed-capacity storage without allocation.

## Acceptance

The dedicated gate was run serially with the QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 5 --cpus 1` — PASS,
  exit 33, `[S23.5] WovenAI semantic search: PASSED`; evidence:
  `audit-artifacts/stage23.5-1cpu-1792524571689446000`
- `python scripts/test-stage23-1-ai-broker.py --stage 5 --cpus 2` — PASS,
  exit 33, `[S23.5] WovenAI semantic search: PASSED`; evidence:
  `audit-artifacts/stage23.5-2cpu-1792524589227010700`
- `python scripts/test-stage23-1-ai-broker.py --stage 5 --cpus 4` — PASS,
  exit 33, `[S23.5] WovenAI semantic search: PASSED`; evidence:
  `audit-artifacts/stage23.5-4cpu-1792524606220735100`

## Static and host checks

- `cargo test -p libwoven --lib` — 76 passed.
- Stage 23.5 freestanding kernel Clippy with `-D warnings` — pass.
- Host all-targets Clippy — pass.
- Python syntax validation and `git diff --check` — pass.

## Limits and next prerequisites

The accepted core does not yet ingest durable filesystem/message sources,
provide vector embeddings, evaluate ranking quality, or expose a user-facing
search service. Those remain required follow-up gates before claiming complete
semantic search.
