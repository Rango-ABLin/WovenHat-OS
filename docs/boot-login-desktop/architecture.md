# Boot, login and desktop architecture

Status: IN PROGRESS through Phase E, 2026-11-10. Phase A inventory, Phase B
software/QEMU boot recovery, Phase C WovenInit readiness, Phase D splash
handoff, and Phase E authentication infrastructure are accepted.

Reconciled with integration d21807ba via local merge aeb9d40a. Direct diff
review found no changes since the initial audit in session.rs, accounts.rs,
identity_runtime.rs, entropy.rs, secure_boot.rs or root build.rs. Startup
main.rs now tolerates absent GOP framebuffer for later GPU attachment, and
storage routing supports WovenFS as the system filesystem. The integration
ledger reports graphics acceptance; this unit's new Windows baseline passed
independently. Historical modernization-open claims below describe
the initial local revision and are superseded by the integration ledger.

The host executable in `src/main.rs` prints the generated image path. It is
not the operating system. Root `build.rs` obtains the freestanding kernel
artifact and calls `bootloader::UefiBoot::create_disk_image`. The patched
`vendor/bootloader/build.rs` initially installed the version-matched registry
`bootloader-x86_64-uefi` crate for `x86_64-unknown-uefi`. B1 now retains that
runtime under `vendor/bootloader/uefi`, with bounded strict configuration
rejection. Its mapping/BootInfo/dependency-version contracts are preserved;
full Phase B qualification remains open. The host wrapper
`vendor/bootloader/src/uefi/mod.rs` must not be mistaken for firmware code.
The resolved dependency was inspected; executable negative boot tests remain
Phase B work.

The resolved local dependency is `bootloader-x86_64-uefi-0.11.17`, under
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. Its `src/main.rs`
has the `#[entry]` firmware function. It loads `kernel-x86_64` from disk
with a TFTP fallback, decodes optional boot configuration, initializes GOP,
loads the ramdisk, calls `boot::exit_boot_services(None)`, sorts the memory
map, obtains ACPI2/ACPI RSDP, and calls the common kernel mapper/transfer.
Malformed configuration currently logs a warning and uses defaults. The
directive requires rejection, so this behavior needs a separately gated fix.

`kernel/src/main.rs` registers `kernel_main` with `bootloader_api::entry_point!`
and receives `BootInfo`. Its production boot path initializes storage, imports
persistent accounts/settings, and calls `session::start` with the embedded
WovenShell ELF. `kernel/src/session.rs` constructs bounded argv and starts
the shell as a system service. It records the process identifier; `active()`
also requires compositor ownership. Process spawn and display ownership are
not proof of successful authentication or an interactive frame.

WovenShell combines sign-in, lock UI, and desktop in
`apps/woven-shell/src/main.rs`, but it receives only the public bounded account
name catalog. Plaintext credentials are copied through the bounded identity ABI
and verified by the kernel authority; verifier-bearing account/WID2 files are
not userspace-readable. `identity_runtime::apply_view` assigns task credentials
only after successful authentication. Digest-replay syscall numbers remain
reserved and fail closed.

The installer is the first-account provisioning authority. It obtains salts
from the kernel secure-entropy boundary and publishes durable account and WID2
state. A production installed boot with missing or invalid account state stops
before starting WovenShell; only the explicitly feature-gated hardware demo may
defer account installation. Phase E accepts this authentication substrate, not
the later graphical login/session/desktop phase behavior.

The fallback `userspace::create_init_process` still provides the diagnostic
assembly stub as `/init`; it is not used as proof of production readiness.
Production system services now start through `kernel/src/woven_init.rs`.
WovenInit owns a bounded four-node dependency graph: package has no
prerequisite, while update, installer, and search each require package to be
ready. A node becomes ready only after its expected PID is alive and owns the
expected IPC service with nonzero client rights. Each readiness wait is
bounded to 500 timer ticks. A spawn, registration, dependency, or readiness
failure kills every process started by that transaction in reverse order;
the existing supervisor takes over restart policy only after the barrier.
WovenShell remains outside this essential-service graph and starts after the
barrier, preserving the later login/session phase boundary.

There is already a splash: `kernel/src/console.rs::show_splash_frame`, driven
by `kernel_main`'s `boot_stage`. Its spinner phase uses timer ticks and its
serial profile reports per-stage tick deltas. The labels say BOOT LOADER
but execute in the kernel. Those deltas are not firmware-to-kernel timing.
Do not create a second splash without evaluating this renderer and ownership.

The production path initializes paging/topology/physical memory from BootInfo,
then terminal/console, heap and compositor, GDT/interrupts/task state and SMP,
then I/O workers/pager, storage/VFS and files, optional network, driver bring-up,
GPU attachment and late storage-root discovery. Live-log startup is deferred.
Display acquisition is waited on for 500 ticks; a still-live shell retains
ownership even when that wait expires. Failure/fallback paths create the
embedded init and expose the diagnostic shell; they need policy review.

Phase A's inventory and baseline gate is closed. The prescribed Windows chain,
Stage12.1–12.5 matrices and affected feature lints passed. Security findings
remain open requirements for later phases, not accepted production behavior.

## Phase B and C accepted boundaries (2026-11-09)

Phase B validates the versioned bounded boot-policy handoff, strict graphics
and firmware/resource failures, and read-only recovery routing. Phase C adds
no syscall, capability, kernel object, lock, ABI, unsafe block, or asynchronous
userspace pointer. WovenInit performs process operations outside the service
registry lock and uses the scheduler/timer-driven service publication path.
The production graph passed normal desktop boots and the deterministic
post-package update failure passed complete rollback at 1/2/4 CPUs. The full
preservation chain passed in
`audit-artifacts/acceptance-20261109-131738-437`.

## Phase D splash, timing, and ownership boundary (2026-11-09)

The production splash remains the existing early integer renderer in
`kernel/src/console.rs`; no second graphics stack was introduced. While it is
visible, the text terminal owns the boot framebuffer. Kernel boot stages now
say `KERNEL BOOT`, and the pre-timer graphics stage is explicitly labeled
`timing=pre-timer`. Delta and total tick measurements begin only after
`timer::init`, at the documented 10 ms resolution, so they do not claim
firmware-to-kernel timing.

WovenShell retains the established `DisplayIo` plus exclusive compositor-lease
authority. Its first successful full-frame presentation atomically transfers
scanout and input focus from the terminal; the evidence marker is emitted only
after releasing the compositor lock. The Phase D harness captures and validates
the branded terminal-owned splash, waits for that real owner-bound first frame,
captures the desktop, and requires a material replacement at unchanged geometry.
