# Stage 24.60 release-key provisioning boundary — 2026-10-21

## Scope

Production WAPP packaging now fails closed unless both
`WOVENHAT_APP_SIGNING_SEED_HEX` and `WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX` are
explicitly supplied. The packager still permits the deterministic key only
for test/development packaging when production mode is not requested. The
production catalog and package-service launchers propagate an explicit release
mode, and release packaging requests that mode when it has to rebuild the
catalog.

The signing seed is read only from the process environment and is not written
to catalogs, manifests, logs, or output artifacts. The packager derives the
public key and rejects a mismatched trusted public key.

## Verification

- `scripts/test-release-key-provisioning.py`: passed; missing-seed production
  invocation rejected, matching explicit-key invocation succeeded.
- `cargo fmt --manifest-path tools/wapp-packager/Cargo.toml`: passed.
- `cargo clippy --manifest-path tools/wapp-packager/Cargo.toml -- -D warnings`:
  passed.
- `python scripts/package-system-apps.py --production`: passed with explicit
  test credentials used only as a deterministic acceptance fixture; ten WAPPs
  were generated.
- `python scripts/build-package-service.py --production`: passed with the
  same explicit acceptance fixture.
- `package-release.py` now always regenerates the catalog in production mode,
  preventing a stale test-key catalog from being copied into a release.
- `git diff --check`: passed.

## Limits

The acceptance fixture is not a production secret. Release operators must
inject a separately controlled signing seed and matching public key. Hardware
TPM/USB qualification and the repository-wide pre-existing formatting debt
remain separate release gates.
