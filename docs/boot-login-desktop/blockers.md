# Blockers and remaining prerequisites

Status: IN PROGRESS. Phase A is accepted. Phase B1 is accepted after its final matrix and full preservation gate.
Bounded B2 kernel-image validation is accepted at 72be85aa. B4 disk firmware-I/O
failure qualification is accepted at 44f780cf. Full Phase B remains open for
remaining boot policy and failure/trust qualification. Later phases remain
behind the Phase B gate. This file does not claim all feasible software work
has been completed.

## External qualification prerequisites

- Virtual Secure Boot/TPM qualification passed with the provisioned development
  key, enrolled certificate, signed guest image and disposable TPM state.
  Evidence: `audit-artifacts/virtual-trust/qualified-custom-ovmf.json`. This is
  development-chain evidence, not production-key or physical-firmware evidence.
- Physical TPM, GPU/input, removable media and real power-loss qualification
  remain unperformed. No physical device or host firmware was modified.

## Software still required by the directive

- B1 rejects malformed policy, but the supported v1 subset currently covers
  existing logging/framebuffer settings only. Boot entry/timeout, alternate
  kernel path, root selection, recovery policy and command-line forwarding are
  still required. B2 covers selected malformed image structures and B3 covers
  loader-data allocation failure. B4 adds source-bound disposable FAT fixtures
  for a present directory instead of the kernel (`DiskFileIsDirectory`) and a
  broken FAT chain (`DiskRead`), with 1/2/4-CPU and full-Windows acceptance.
  Broader boot-information validation, strict graphics, page-table and
  ExitBootServices failure qualification remain Phase B work. B5 firmware TFTP
  I/O is accepted: real PXE boots cover both `TftpFileSize` and a separate
  deterministic `TftpRead` rejection on 1/2/4 CPUs. The fixture is disposable
  and source-bound; it does not qualify arbitrary network firmware or hardware.
- Kernel readiness and the actual first-userspace WovenInit/service dependency
  model are not implemented by this unit. The embedded /init stub and existing
  kernel-side supervisor must not be called a complete WovenInit architecture.
- Production secure entropy/provisioning is not established: inspect
  kernel/src/entropy.rs and identity_runtime::{admin_add_user,change_pin}, which
  still use the documented best-effort random API for account salts.
- Missing-account startup, publicly readable verifier records/digest replay,
  pre-authentication capabilities, F12 diagnostics and crash fallback need the
  negative security qualifications described in security-model.md. Source
  locations: kernel/src/main.rs::install_persistent_accounts, session.rs,
  identity_runtime.rs, vfs.rs::create_read_only, libwoven/src/identity.rs::
  Credential::verify_digest and apps/woven-shell/src/main.rs.
- Separate login/session readiness, first-run setup, logout/session-owned child
  cleanup, bounded desktop/display-manager recovery and complete graphical
  end-to-end/repeated-boot/performance acceptance are not closed.

## Resolved baseline defects / environment requirements

The initial host compile and formatting failures are repaired and retained.
Phase A full Windows and Stage12.1–12.5 matrices passed at runtime a980619e.
Host libwoven also passes 212 units and package-specific Clippy after the
same-pixel test-index cleanup. Network acceptance needs approved host networking:
restricted execution produced a DNS timeout; the unchanged gate passed outside
that restriction. Cached Cargo dependencies work with CARGO_NET_OFFLINE=true.
Only one Cargo/acceptance chain may run, including across worktrees; concurrent
worktree compilation previously invalidated shared bootloader artifacts.
