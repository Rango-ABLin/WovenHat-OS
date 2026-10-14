# Stage 15.7 WovenVault handle audit — 2026-10-13

The WovenVault ABI now has a bounded volatile secret store. Secrets are copied
into 16 fixed 256-byte kernel slots, addressed by generation-tagged handles,
and are never exposed as kernel pointers. Reads require the owning task and
copy through the validated userspace-memory helpers; revoke and owner teardown
wipe slot bytes with volatile stores. Stale generations, foreign owners,
capacity, empty values, and oversized values fail closed.

The new `Secrets` capability is absent from ordinary userspace defaults but is
inside the userspace sandbox ceiling so a future permission decision can grant
it. Syscalls 127–129 expose store/read/revoke, and `libwoven` provides typed
wrappers.

Validation:

- freestanding kernel Clippy with `-D warnings`: passed;
- 1 CPU: `audit-artifacts/stage15.7-1cpu-1791901065186624800`;
- 2 CPUs: `audit-artifacts/stage15.7-2cpu-1791901104773292200`;
- 4 CPUs: `audit-artifacts/stage15.7-4cpu-1791901119604868100`;
- marker: `[S15.7] WovenVault handles: PASSED`.

This closes the volatile handle and teardown boundary only. Persistent secret
records, encrypted-at-rest storage, rotation policy, TPM/firmware-backed key
provisioning, and a graphical secrets UI remain required for a complete
WovenVault product.
