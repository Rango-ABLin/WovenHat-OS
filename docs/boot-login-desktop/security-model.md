# Security audit

Status: PHASE F ACCEPTED, 2026-11-10. Later session, recovery and
hardware/trust findings remain open until their own gates pass.

The kernel identity authority is the authentication boundary. Plaintext
credentials cross the bounded syscall ABI only after user buffers are copied;
the kernel verifies them against its protected authority state, binds a session
to the calling process, applies credentials only after success, and releases
owner bindings during process teardown. The retired digest-replay syscalls keep
their ABI numbers reserved but return failure without authenticating.

Passwords use PBKDF2-HMAC-SHA256 at 100000 iterations. Argon2id is not
established. Credential salts are generated only by `entropy::fill_secure`,
which requires CPU RDRAND and zeroes the destination on failure; provisioning
and PIN rotation fail closed when that source is unavailable.

Verifier-bearing account and WID2 paths are kernel-protected and cannot be
opened through the userspace VFS ABI. A separate public names catalog supplies
only bounded account names to WovenShell. The shell no longer receives salts or
hashes, and the public libwoven digest helpers fail closed. The Stage 20.12
runtime probe verifies that userspace can read names but cannot read verifier
records.

Accepted Phase E controls:

- The installer creates the initial account with secure entropy and publishes
  the durable account/identity state. An installed production boot with missing
  or invalid account state logs setup-required and halts before WovenShell.
- Unknown and known usernames take the same bounded PBKDF authentication path;
  both feed the fixed-capacity per-account failure table. The configurable
  threshold is durable, while transient failure counts and cooldowns reset at
  boot and are therefore not claimed as durable state.
- Wrong credentials, missing accounts and unavailable entropy cannot create an
  active privileged session. Owner-bound identity and ABI gates pass on 1/2/4
  CPUs, including reserved digest-replay rejection.
- Correct sign-in applies the selected user's identity, lock requires the same
  kernel authority to unlock, and PIN rotation invalidates the old credential.

Phase F qualifies the full graphical login path, including keyboard/pointer
selection, masked PIN entry, failure feedback, high contrast and calls into the
actual kernel authentication service. The login surface consumes only public
account names and does not gain verifier access or local session-minting
authority. Remaining boundaries are deliberately outside Phase F: Phases G/H
must prove session-owned launch authority, inheritance, logout cleanup and
desktop authorization; recovery/F12/fallback behavior remains subject to its
later negative integration gates. Production Secure Boot keys, physical TPM
and native hardware remain final trust/hardware work.

The kernel shell authorizes commands against the current kernel task's
capabilities (`shell::authorize`), not an authenticated desktop session.
The event loop routes keyboard input to it when the desktop is not presenting,
including after compositor process failure. This is a concrete recovery-policy
integration point, not a secure text-login implementation.

The historical persistent-account fixture was still writing a single-SHA256
verifier incompatible with the current KDF. BLD-A2 corrects this disposable
test-media generator only. Its fixed, public fixture credentials are not a
production account-creation path or evidence of secure entropy/provisioning.


B2 adds bounded structural kernel inspection before the upstream common mapper.
It rejects selected malformed ELF and embedded boot-contract inputs and caps
raw file allocation. It does not authenticate image origin or validate every
relocation, mapping or runtime semantic. A structurally valid malicious kernel
is not rendered safe by these checks. Existing software signed-handoff and
firmware Secure Boot qualification remain separate gates; no trust anchor or
production key is added. No kernel capabilities or identity behavior change.
