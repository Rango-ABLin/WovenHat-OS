# WovenHat musl provenance

This directory contains an unmodified source archive of upstream musl libc
`v1.2.6`, imported from the official repository at
`https://git.musl-libc.org/git/musl`.

- Upstream commit: `9fa28ece75d8a2191de7c5bb53bed224c5947417`
- Upstream tag: `v1.2.6`
- License: MIT; the upstream [`COPYRIGHT`](COPYRIGHT) file is retained
- `COPYRIGHT` SHA-256:
  `78b7299d4ca6d7cb8927e957b83e4bc85fd3a86f2c2c97f8b802a34ac512d775`

The archive intentionally contains no nested `.git` directory. WovenHat port
changes must live outside upstream source paths where possible and must record
their ABI and conformance impact before the resulting libc is accepted.
