# Source inventory

Status: IN PROGRESS. Paths below were discovered in this checkout.

| Component | Source / integration |
|---|---|
| Image construction | build.rs; vendor/bootloader/src/uefi/mod.rs |
| Firmware dependency build | vendor/bootloader/build.rs |
| B1 retained firmware entry and bounded policy parser | vendor/bootloader/uefi/src/main.rs; vendor/bootloader/uefi/src/config.rs |
| Disposable boot policy image producer and QEMU gate | tools/boot-config-fixtures; scripts/test-boot-config-qemu.py |
| Kernel BootInfo entry and orchestration | kernel/src/main.rs, kernel_main |
| Physical / virtual memory | kernel/src/memory.rs; kernel/src/paging.rs |
| CPU descriptors and interrupts | kernel/src/gdt.rs; kernel/src/interrupts.rs |
| ACPI, CPU/PCI discovery, legacy IRQs and timer | kernel/src/hal/acpi.rs; kernel/src/hal/cpu.rs; kernel/src/hal/pci.rs; kernel/src/pic.rs; kernel/src/timer.rs |
| SMP and scheduler/process authority | kernel/src/smp.rs; kernel/src/task.rs |
| ELF and fallback init | kernel/src/userspace.rs; kernel/src/userspace/linker.rs |
| Syscall boundary | kernel/src/syscall.rs |
| IPC | kernel/src/ipc.rs |
| Filesystem mount and namespace | kernel/src/storage.rs; kernel/src/vfs.rs |
| FAT32 and native secondary filesystem | kernel/src/fat32.rs; kernel/src/wfs.rs |
| Display and composition | kernel/src/display.rs; kernel/src/compositor.rs |
| Input fan-in | kernel/src/woven_input.rs |
| Existing boot splash | kernel/src/console.rs, show_splash_frame; kernel/src/main.rs, boot_stage |
| Session bootstrap | kernel/src/session.rs |
| Login and desktop UI | apps/woven-shell/src/main.rs |
| Account verification and authority | kernel/src/accounts.rs; kernel/src/identity_runtime.rs |
| Durable authority | kernel/src/identity_storage.rs |
| Randomness | kernel/src/entropy.rs |
| Existing service supervision | kernel/src/service_supervisor.rs |
| Recovery debug shell | kernel/src/shell.rs; kernel/src/main.rs production event loop |
| Windows preservation gate | RUN-STAGE10.7.ps1; run-stage10-7-acceptance.ps1 |
| Desktop framebuffer journey | scripts/test-desktop-qualification.py |
| Session QEMU harness | scripts/test-stage20-15-session.py |
| Persistent account test-media generator | scripts/create-account-fixture.py; tests/test_account_fixture.py |
| Repeated boots | scripts/test-stage31-repeated-boots.py |
| CI | .github/workflows/kernel.yml; wovenhat-windows-acceptance.yml; hardware-qualification.yml; wovenhat-runner-test.yml |

Integration revision d21807ba supersedes the old filesystem decision: the
recorded 2026-11-03 user instruction makes WovenFS the system filesystem and
keeps FAT32 for EFI/removable media. The installed-image software gate is
reported passing at 1/2/4 CPUs in the filesystem migration tracker. Preserve
that integration; do not restore the old primary-FAT32 policy. WovenWeb
remains paused. Detailed shutdown and first-run provisioning traces remain
outstanding.
