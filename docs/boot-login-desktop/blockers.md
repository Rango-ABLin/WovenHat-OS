# Blockers and remaining prerequisites

Status: IN PROGRESS through Phase E. Phases A-E are accepted in software/QEMU;
Phase F is the next software gate. Physical firmware, TPM, GPU/input, removable
media and native qualification remain deferred to the final hardware pass.

## External qualification prerequisites

- Virtual Secure Boot/TPM qualification passed with the provisioned development
  key, enrolled certificate, signed guest image and disposable TPM state.
  Evidence: `audit-artifacts/virtual-trust/qualified-custom-ovmf.json`. This is
  development-chain evidence, not production-key or physical-firmware evidence.
- Physical TPM, GPU/input, removable media and real power-loss qualification
  remain unperformed. No physical device or host firmware was modified.

## Software still required by the directive

- Phase F must qualify the graphical login itself, including input focus,
  masking, accessibility, failure presentation and real authority use.
- Phases G/H must prove authenticated session ownership, launch inheritance,
  replay/forgery rejection, logout cleanup and desktop authorization. Existing
  pre-login shell capabilities are not accepted as a completed session model.
- Phase I must cover repeated login/logout/reboot/crash/persistence journeys and
  recovery/F12/fallback negative cases without an authentication bypass.
- Phase J must produce the reproducible release candidate, CI/launcher and
  complete limitations record. Physical/trust items remain separate below.

## Resolved baseline defects / environment requirements

The initial host compile and formatting failures are repaired and retained.
Phase A full Windows and Stage12.1–12.5 matrices passed at runtime a980619e.
Host libwoven also passes 212 units and package-specific Clippy after the
same-pixel test-index cleanup. Network acceptance needs approved host networking:
restricted execution produced a DNS timeout; the unchanged gate passed outside
that restriction. Cached Cargo dependencies work with CARGO_NET_OFFLINE=true.
Only one Cargo/acceptance chain may run, including across worktrees; concurrent
worktree compilation previously invalidated shared bootloader artifacts.
