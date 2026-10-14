# Stage 15.4 signed applications — cryptographic boundary accepted (2026-10-13)

The signed-image cryptographic boundary is accepted on 1/2/4 CPUs. The kernel
now uses the pure-Rust, `no_std` `ed25519-compact` implementation and validates
a bounded versioned `WAPP` envelope before exposing its immutable payload.
Malformed envelopes and payload tampering fail closed.

## Investigation

The first Ed25519 attempt used `ed25519-dalek`; its freestanding compile
reached `curve25519-dalek`, which selected its x86_64 SIMD backend and failed
in the pinned nightly LLVM with:

`rustc-LLVM ERROR: Do not know how to split the result of this operator!`

That dependency was removed. The replacement has no C/assembly build step and
passes warning-denying freestanding Clippy. The 1/2/4-CPU runtime evidence is:

- `audit-artifacts/stage15.4-1cpu-1791903175601544700`
- `audit-artifacts/stage15.4-2cpu-1791903335400954700`
- `audit-artifacts/stage15.4-4cpu-1791903444701949100`

After application trust-root provisioning was added, the final revalidation
also passed:

- `audit-artifacts/stage15.4-1cpu-1791957273567583900`
- `audit-artifacts/stage15.4-2cpu-1791957346171841400`
- `audit-artifacts/stage15.4-4cpu-1791957408773610900`

After signed external-application loading was tightened, the prior Stage 15.4
gate was rerun on the final loader boundary:

- `audit-artifacts/stage15.4-1cpu-1791961845238807400`
- `audit-artifacts/stage15.4-2cpu-1791961914527257900`
- `audit-artifacts/stage15.4-4cpu-1791961962399446600`

All three exited 33 with the required marker.

## Remaining acceptance requirements

- immutable firmware/installer-owned trust-root provisioning and
  rotation/revocation policy;
- enforcement before every executable load, including all internal launch
  paths, `exec`, and shell launch;
- rollback metadata, key rotation, and production signed-package tooling;
- refreshed 1/2/4-CPU evidence for the final loader integration.

Historical boundary note: the WAPP boundary defaulted to the development
application trust root for test images, while release builds now require the compile-time
`WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX` provisioning value. The RFC self-test is
kept independent of the release key. Firmware/installer-owned key slots,
package rollback metadata, and release package tooling remain open.

The external-application pipeline now includes `tools/wapp-packager`, which
signs the Stage 16.1 ELF before the kernel artifact is compiled. The Stage 16.1
1/2/4-CPU gate verifies that package before ELF parsing and again after the VFS
round trip. This closes the previously missing signed-package integration for
that acceptance path. User-triggered loads reject raw external ELF except for
the finite kernel-owned system-image allowlist. The ten-application catalog,
rollback-safe RAM publication, release packaging, and production system-app
boot are now covered by the current Stage 19 release chain.
Installer/firmware-owned key slots, deployment-key rotation/revocation, and
physical rollback qualification remain open.
Kernel-internal system images remain explicitly trusted system paths;
user-controlled `exec`, command, and spawn loads use the signed-package
boundary.
