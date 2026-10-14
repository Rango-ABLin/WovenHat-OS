# Stage 19 multi-application catalog audit — 2026-10-14

## Scope

The feature-gated `stage19-app-catalog-test` acceptance consumes the output of
`scripts/package-system-apps.py` and exercises the four accepted external
applications as one install plan:

- WovenCalculator at `/bin/wovenhat-calculator`;
- WovenTerminal at `/bin/wovenhat-terminal`; and
- WovenFiles at `/bin/wovenhat-files`; and
- WovenMonitor at `/bin/wovenhat-monitor`.

Each package is verified as WAPP v1, mutated-package refusal is checked, the
package is written to and read back from the VFS byte-for-byte, and the
read-back payload is passed through the normal verified ELF loader. The four
generated `/share/applications/*.app` descriptors are published only after the
corresponding package has passed those checks.

## Acceptance

`scripts/test-system-app-catalog-qemu.py` passed sequentially on 1, 2, and 4
virtual CPUs, each with exit 33 and the marker
`[S19-APP] four signed applications installed PASS`. Serial and QEMU logs are
preserved in `audit-artifacts/system-app-catalog-1cpu`,
`audit-artifacts/system-app-catalog-2cpu`, and
`audit-artifacts/system-app-catalog-4cpu`.

The kernel feature also passes warning-denying freestanding Clippy. The
packaging command was rerun for the acceptance build and produced the same
bounded package sizes and manifest structure.

Repository-wide `cargo clippy --all-targets -- -D warnings` also passes. A
repository-wide `cargo fmt --all -- --check` reports formatting drift in
pre-existing files outside this increment; those unrelated files were not
rewritten during this acceptance so historical code and evidence remain
unchanged.

## Limits

This catalog gate is separate from the feature-gated production-image boot
installer. That installer now performs bounded pre-verification, package-first
publication, and rollback of entries and directories created by the current
RAM-backed transaction. Persistent-media installation, power-loss recovery,
key rotation/revocation, and native hardware qualification remain open
requirements. No removable media was modified.
