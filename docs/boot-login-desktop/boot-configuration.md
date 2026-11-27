# B1 boot configuration rejection

Status: TESTED — PASS (B1 bounded unit), runtime aaeaaf8f. Phase B is not complete.

The existing UEFI implementation is vendored at
`vendor/bootloader/uefi`, with its mapper, BootInfo and runtime dependency
versions preserved. Its new parser is also compiled directly by
`tests/boot_config.rs`; host validation is not a replacement for firmware tests.

`boot.json` accepts known upstream logging/framebuffer fields, plus an optional
`version` whose only accepted value is 1. Omitted version preserves legacy
settings. The maximum file size is 4096 bytes, checked from disk/TFTP metadata
before allocating or reading the payload. Empty files, malformed JSON,
duplicates, unknown fields, unsupported versions and trailing non-JSON data
are rejected. Missing configuration retains the established defaults.

Example of the supported v1 subset:

```json
{
  "version": 1,
  "serial_logging": true,
  "frame_buffer_logging": true,
  "log_level": "Info",
  "frame_buffer": {
    "minimum_framebuffer_width": 1280,
    "minimum_framebuffer_height": 800
  }
}
```

Framebuffer dimensions retain upstream preference/fallback semantics; this
does not introduce strict mode selection. Boot entries/timeouts, alternate
kernel paths, root selection, recovery policy and kernel command-line forwarding
remain unimplemented. Those unknown fields fail closed; they are not presented
as working options. Configuration integrity is not authenticated boot.

Parse errors establish a default diagnostic console solely to show the error,
then halt before ramdisk loading or ExitBootServices. Size failures use the
existing firmware-console panic path before the framebuffer logger exists.
Both paths report static error classes, never untrusted configuration contents.

No new kernel object, capability, syscall, lock or hand-written unsafe block
is added. Deserialization produces owned scalar fields; no untrusted pointer
or borrowed policy survives into asynchronous work. New size checks apply only
to configuration; kernel/ramdisk ownership, loading and teardown are unchanged.
The existing loader's unsafe page-table and firmware operations remain upstream
code. Disk QEMU qualification does not establish live PXE/TFTP or physical
firmware qualification.

Reproduction (project-local Cargo directories and cached dependencies):

```powershell
$env:CARGO_BUILD_BUILD_DIR = Join-Path $PWD '.cargo-build'
$env:CARGO_TARGET_DIR = Join-Path $PWD 'target'
$env:CARGO_NET_OFFLINE = 'true'
cargo test --test boot_config
cargo clippy --all-targets -- -D warnings
cargo clippy --manifest-path vendor/bootloader/uefi/Cargo.toml --target x86_64-unknown-uefi -- -D warnings
python scripts/test-boot-config-qemu.py --prepare --fixtures audit-artifacts/boot-config-final-images --cpus 1
python scripts/test-boot-config-qemu.py --fixtures audit-artifacts/boot-config-final-images --cpus 2
python scripts/test-boot-config-qemu.py --fixtures audit-artifacts/boot-config-final-images --cpus 4
```

Use a new fixture directory for every preparation; existing inputs/evidence
are never overwritten. Every CPU profile uses the exact same image hashes.
Positive cases require QEMU exit 33, kernel completion and exact online CPU
count. Rejections require the exact error and controlled-stop markers, a live
guest in the failure path, no ExitBootServices/kernel handoff, and framebuffer capture.
The manifest records source/kernel/image hashes for provenance, not a trust
anchor. Test processes are the only QEMU processes the harness may terminate.

Initial 1-CPU qualification caught a trailing-data diagnostic mismatch. The
parser mapping and host assertion were corrected without relaxing the oracle;
the failure is retained in `audit-artifacts/boot-config-b1/1cpu.log` and its
timestamped serial directory. The next 1-CPU run passed all nine cases,
including empty/oversized preallocation rejection. Captured malformed and
oversized screens were visually inspected and show explicit boot-stopped
errors, with no successful splash. Final source-bound qualification passes
all 27 cases at 1/2/4 CPUs; evidence is under audit-artifacts/boot-config-b1/
final-{1,2,4}cpu.log and the recorded timestamped case directories.
Stage15.5 signed-handoff passes at 1/2/4 CPUs. Full Windows preservation also
passes at aaeaaf8f in audit-artifacts/acceptance-20261104-205802-369/.
