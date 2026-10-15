# Cross-subsystem completion register — WovenFS, WovenGuard and platform

Status: **AUDIT / NOT ACCEPTED**. Baseline: accessible `master` commit `a54ae319` (Stage 15.2). The user reports a newer Stage 19.7 tree, which is **not present in the connected branch**. Reconcile this register against that newer tree before implementation or merging.

## P0 — evidence and source reconciliation
- [ ] Locate and compare the Stage 19.7 source branch and exact SHA; do not overwrite later work with Stage 15-era changes.
- [ ] Require current warning-denying Clippy, host tests, 1/2/4 CPU QEMU acceptance, and retained failure artifacts for every affected feature.
- [ ] Keep this audit branch independent of the older Stage 15.3 and shared-surface draft PRs.

## P1 — WovenFS and storage
- [ ] Integrate native `wfs.rs` through the normal typed VFS, with explicit mount namespace and access checks; `wovenfs.rs` is the legacy in-memory metadata layer, not the on-disk implementation.
- [ ] Add exclusive writable volume ownership, mount/unmount lifecycle, stale handle refusal, and isolation between FAT32 and WovenFS.
- [ ] Test VFS create/read/write/unlink across unmount/remount/reboot and corrupt-volume refusal.
- [ ] Preserve and extend exhaustive journal crash-boundary tests through the VFS adapter; verify recovery idempotence and no partial visible commits.
- [ ] Specify and test large-file/indirect-block evolution beyond the current 4 KiB file limit and 64-entry directory limit; version on-disk changes rather than silently changing the v1 ABI.
- [ ] Qualify physical power-loss, flush/FUA guarantees, DMA, and filesystem recovery on authorized test hardware. QEMU-only results cannot close this requirement.
- [ ] Address bounded FAT32 metadata capacity and batch transaction limits without weakening WDJ1/WDJ2 or WMD compatibility.

## P1 — WovenGuard
- [ ] Audit every syscall, IPC handle transfer, filesystem operation, DMA mapping, and surface mapping for capability checks at the actual enforcement boundary.
- [ ] Verify security-domain and sandbox ceilings for delegated rights; reject privilege escalation on handle duplication/transfer.
- [ ] Exercise recursive revocation, generation reuse, task exit, and races with in-flight I/O and compositor frame publication on 1/2/4 CPUs.
- [ ] Verify denial auditing cannot panic, allocate unbounded memory, or recurse into security locks.
- [ ] Extend firewall policy administration and packet enforcement to all supported transports; test bootstrap-to-enforcement transitions and malformed packets.
- [ ] Establish signed/verified service identities and a documented policy for privileged services before production-readiness claims.

## P1 — shared-memory windows and userspace
- [ ] Implement capability-scoped surface allocation/mapping, exclusive writer vs compositor reader, frame ownership, and explicit synchronization.
- [ ] Revoke mappings and reject stale generation/sequence numbers on window close, process exit, and service restart.
- [ ] Expand `libwoven` beyond its currently documented minimal syscall wrappers so a userspace compositor can actually run.
- [ ] Keep compositor and window manager in userspace; do not extend the kernel's GUI primitives into an unrestricted desktop server.

## P2 — platform and release
- [ ] Add/maintain CI gates for stages 6–12 and every security-critical feature configuration.
- [ ] Complete physical Wi-Fi/Bluetooth, IPv6 live traffic, storage, interrupt and DMA qualification.
- [ ] Document and justify every `unsafe` block; prioritize paging, SMP, task, userspace, and VirtIO networking.
- [ ] Add installation, recovery, update, release signing, supported hardware, and rollback tests.

## Acceptance rule
An item is **complete** only when the current source implements it, its negative and concurrent tests pass, its acceptance artifacts identify exact SHA and run IDs, and the architecture guide is updated. This register is not evidence that any item passed.
