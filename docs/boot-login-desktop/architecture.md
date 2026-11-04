# Boot, login and desktop architecture

Status: COMPLETE (Phase A inventory only), 2026-11-04. Initial source audit
at 70b49c74; reconciled integration and accepted runtime baseline a980619e.

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

WovenShell currently combines sign-in, lock UI, and desktop in
`apps/woven-shell/src/main.rs`. Account-backed sign-in computes a digest in
userspace and requests `session_open_verified`; the syscall copies bounded
input, checks AccountManage, and delegates to kernel identity authority.
`identity_runtime::apply_view` assigns task credentials. This is an existing
architecture to audit and extend, not grounds to create a competing stack.

The fallback `userspace::create_init_process` loads an embedded assembly stub
as `/init`; it is not the requested userspace dependency supervisor.
`kernel/src/service_supervisor.rs` provides bounded kernel-side supervision
for four service kinds, but excludes WovenShell. No separate WovenInit,
WovenLogin, or WovenSession application was found in the app inventory.

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
