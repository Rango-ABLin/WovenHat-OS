# Stage 15.5 secure boot — signed handoff accepted (2026-10-13)

The kernel now performs a fail-closed signed handoff check before mutable
allocator, paging, driver, or userspace initialization. The manifest signs a
canonical executable PT_LOAD representation sourced from the bootloader's
immutable kernel ELF range. The boot trust key is separate from the Stage
15.4 application key. Release builds now require both
`WOVENHAT_BOOT_SIGNING_SEED_HEX` and the matching
`WOVENHAT_BOOT_TRUSTED_PUBLIC_KEY_HEX`; the image builder rejects a mismatch.
The negative provisioning check was exercised on 2026-10-14 with the final
public-key byte changed; Cargo failed in `build.rs` with
`boot signing seed/public key mismatch` before producing an image.

Acceptance passed with the exact current source and required CPU counts:

- `audit-artifacts/stage15.5-1cpu-1791910199679516200`
- `audit-artifacts/stage15.5-2cpu-1791910271627070600`
- `audit-artifacts/stage15.5-4cpu-1791910414693161200`

After release-key provisioning was added, the final revalidation also passed:

- `audit-artifacts/stage15.5-1cpu-1791911220669252100`
- `audit-artifacts/stage15.5-2cpu-1791911441869901500`
- `audit-artifacts/stage15.5-4cpu-1791911631937944200`

The current bootloader API exposes the loaded kernel range and memory map, but
does not expose UEFI Secure Boot state, authenticated variable access, or a
TPM/event-log interface to the kernel. Firmware enforcement, signed-initrd
policy, revocation, and rollback protection remain platform requirements.

This accepts the software signed-handoff boundary, not full platform Secure
Boot. The current bootloader API does not expose UEFI Secure Boot state,
authenticated variable access, TPM/event-log interfaces, release-key
provisioning, signed-initrd policy, revocation, rollback protection, or
negative tamper/rollback evidence. Those firmware, provisioning, and hardware
qualification requirements remain open.

Current status: the software signed-handoff gate is accepted; the title's
platform-level “secure boot” wording remains intentionally conservative.
# Stage 15.5 secure boot — signed handoff accepted (2026-10-13)

