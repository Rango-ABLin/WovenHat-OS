# WovenUpdate package producer

`wovenhat-wupd-packager INPUT OUTPUT GENERATION` creates a bounded version-1
`WUPD` envelope for the kernel update verifier. The envelope contains the
generation, image length, SHA-256 image digest, and an Ed25519 signature over
the domain separator, immutable header, and exact image bytes.

Development runs use the deterministic fixture key when no signing variables
are present. Release runs must set `WOVENHAT_APP_RELEASE_MODE=1`,
`WOVENHAT_APP_SIGNING_SEED_HEX`, and the matching
`WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX`; mismatches fail closed.

This tool produces a signed candidate only. It does not install an image,
select a boot slot, persist update state, or perform rollback. Those remain
the responsibility of the not-yet-complete privileged update service and boot
integration.
