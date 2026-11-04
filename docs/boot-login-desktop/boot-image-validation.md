# B2 kernel-image boundary (bounded unit accepted)

B1 remains accepted at aaeaaf8f. B2 adds a 256 MiB raw kernel file bound before
allocation (the accepted debug ELF is approximately 67 MiB), rejects empty
payloads, and provides explicit firmware-console rejection before handoff.
Disk and TFTP use the same size policy. Missing disk kernels retain the existing
TFTP fallback, then fail explicitly if neither source provides a kernel.

The allocation-free safe Rust inspector validates supported ELF64 little-endian
x86_64 ET_EXEC/ET_DYN headers; checked file ranges; program/section table sizes;
PT_LOAD file/memory bounds and alignment; entry in a file-backed executable
segment; bounded section names; a unique PROGBITS .bootloader-config; and the
upstream BootloaderConfig deserializer. The existing xmas-elf object, common
mapper and BootInfo are reused. xmas-elf was already pinned transitively; its
new direct dependency edge does not upgrade packages. No hand-written unsafe
code, kernel object, authority or capability is added.

This does not authenticate a kernel, implement firmware Secure Boot, validate
all dynamic relocation semantics, establish recovery policy, or close Phase B.
Existing upstream I/O/allocation failures still use their original diagnostics.
A structurally valid malicious image is outside this increment's guarantee.

Host cases cover both executable kinds, wrong architecture, short headers,
overflowed table offsets, entry outside executable bytes, invalid segment sizes,
address overflow, alignment, missing contract, invalid string table index/name,
and unsupported boot contract. QEMU fixtures retain all B1 cases and add missing,
empty, truncated, corrupt, wrong-architecture and invalid-table kernel payloads.
Negative oracles require explicit kernel failure/stop markers, a live stopped
guest and no ExitBootServices or kernel marker. Source/image hashes and serial,
QEMU command and framebuffer evidence are preserved by the existing harness.

Status: all 45 actual UEFI cases pass on 1/2/4 CPUs at runtime 72be85aa;
evidence is listed in test-results.md and audit-artifacts/boot-image-b2/.
Missing-kernel firmware framebuffer capture was visually inspected and shows
explicit invalid-image and boot-stopped messages. Full Windows preservation
passes in audit-artifacts/acceptance-20261104-214457-697/ (214 Python tests,
six skips, host Rust tests, lint/build checks, 60 memory boots, networking and
isolated Stage10.4-10.7 at 1/2/4 CPUs). Stage15.5 signed-handoff and its kernel
feature Clippy pass; evidence is in boot-image-b2/signed-handoff-{1,2,4}cpu/.
Root/UEFI/fixture-builder warning-denying Clippy and format checks pass; all
8 Rust parser tests and 6 Python harness tests pass. Package version/checksum
sets are unchanged (audit-artifacts/boot-image-b2/pinned-dependency-audit.json).
The bounded B2 unit is accepted; full Phase B remains open.
