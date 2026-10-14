# Stage 16.1 out-of-kernel application gate — accepted (2026-10-14)

The acceptance harness now exposes Stage 16.1 directly. It builds
`apps/hello` outside the kernel workspace, signs the resulting ELF into a WAPP
package, embeds that package through `WOVENHAT_APP_IMAGE`, verifies it before
ELF parsing, installs it into the VFS, reads it back, loads the verified
filesystem bytes, spawns it in Ring 3, and verifies completion and exit status.

Final 1/2/4-CPU evidence:

- `target/application-1cpu`
- `target/application-2cpu`
- `target/application-4cpu`

All three runs passed with exit 33 and markers `[S16.1] signed WAPP package
verifies PASS`, `[S16.1] tampered WAPP package is refused PASS`,
`[S16.1] unsigned external image is refused PASS`, and
`[S16.1] out-of-kernel application: PASSED`.

The package format is produced by `tools/wapp-packager`; release use can set
`WOVENHAT_APP_SIGNING_SEED_HEX` and
`WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX`. The acceptance harness intentionally
uses the fixed development vector and does not provision a production key.

As a separate provisioning check, the same 1-CPU run was repeated with an
explicit non-test signing seed and matching trusted public key. It passed the
same verification, tamper refusal, filesystem, execution, and exit checks.
The private seed is not recorded in the repository or audit documents.

The gate does not claim a package manager, multi-application image assembly,
FAT32 reboot persistence, shared libraries, TLS, or stack-size negotiation;
those remain explicit platform requirements.
