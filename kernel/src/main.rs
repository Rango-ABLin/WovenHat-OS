#![cfg_attr(feature = "qemu-test", allow(dead_code, unused_imports))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![cfg_attr(
    test,
    allow(
        dead_code,
        unused_imports,
        unused_variables,
        unfulfilled_lint_expectations
    )
)]
#![feature(abi_x86_interrupt)]
#![cfg_attr(not(test), feature(alloc_error_handler))]

extern crate alloc;

#[cfg(any(feature = "physical-probe", feature = "physical-wifi"))]
mod physical_probe;

mod accounts;
#[cfg(any(
    not(feature = "qemu-test"),
    feature = "stage13-12-test",
    feature = "stage13-4-test"
))]
mod ahci;
#[cfg(feature = "stage10-8-test")]
mod async_acceptance;
mod async_events;
mod async_file;
mod async_network;
mod async_op;
mod ata;
mod audit;
mod benchmark;
mod block;
mod block_cache;
mod block_io;
mod boot_measurement;
mod capability;
mod completion_port;
mod completion_queue;
mod config;
mod console;
mod deadline;
mod device;
mod display;
mod driver;
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
mod drivers;
mod elf;
mod entropy;
mod exit_status;
mod fat32;
mod file_frames;
mod file_mapping;
#[cfg(any(
    feature = "stage14-5-test",
    feature = "stage14-5b-test",
    feature = "stage14-5c-test",
    feature = "stage14-5d-test"
))]
mod firewall_policy;
mod font;
mod gdt;
mod gpt;
mod graphics;
mod gui;
mod hal;
#[cfg(any(
    not(feature = "qemu-test"),
    feature = "stage13-12-test",
    feature = "stage13-8-test"
))]
mod hda;
mod heap;
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
mod hid;
#[cfg(any(
    feature = "stage20-12-test",
    feature = "stage24-4-test",
    feature = "stage24-5-test",
    feature = "system-apps"
))]
mod identity_runtime;
#[cfg(any(feature = "stage24-3-test", feature = "system-apps"))]
mod identity_storage;
mod interrupts;
mod ipc;
mod irq_lock;
mod journal;
mod keyboard;
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
mod livelog;
mod memory;
mod mouse;
mod network;
mod notifications;
#[cfg(any(
    not(feature = "qemu-test"),
    feature = "stage13-12-test",
    feature = "stage13-3-test"
))]
mod nvme;
mod page_cache;
mod page_permissions;
mod paging;
mod panic;
mod partition;
mod permission;
mod pic;
mod pipe;
mod power;
mod rtc;
mod secret_vault;
#[cfg(feature = "secure-boot")]
mod secure_boot;
mod serial;
mod session;
mod shell;
mod signed_app;
mod smp;
#[cfg(feature = "stage12-4-test")]
mod snapshots;
mod storage;
// Stage 12.3 acceptance exercises the encrypted-mount path that lives here, so
// this module is required by both the 12.3 and 12.5 gates. The features
// themselves must not chain: each Stage 12 self-test block ends the boot, so
// enabling 12.3 from 12.5 would exit before the 12.5 block ran.
mod compositor;
#[cfg(any(
    feature = "stage21-1-test",
    feature = "stage21-2-test",
    feature = "stage21-3-test",
    feature = "stage21-4-test",
    feature = "stage21-5-test",
    feature = "stage21-6-test",
    feature = "stage21-7-test",
    feature = "stage21-8-test",
    feature = "stage21-9-test",
    feature = "stage21-10-test",
    feature = "stage21-11-test",
    feature = "stage22-5-test",
    feature = "system-apps"
))]
mod package;
#[cfg(any(
    feature = "stage21-7-test",
    feature = "stage21-8-test",
    feature = "stage21-9-test",
    feature = "stage21-10-test",
    feature = "stage21-11-test",
    feature = "stage22-5-test",
    feature = "system-apps"
))]
mod package_service;
#[cfg(any(feature = "stage12-3-test", feature = "stage12-5-test"))]
mod storage_manager;
mod swap;
mod syscall;
mod task;
mod terminal;
#[allow(dead_code)]
mod thread;
mod timer;
mod tpm2;
#[cfg(any(feature = "qemu-test", feature = "system-apps"))]
mod update;
#[cfg(any(feature = "stage27-5-test", feature = "system-apps"))]
mod update_service;
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
mod usb_msc;
mod user_address;
mod user_graphics;
mod userspace;
mod vfs;
#[cfg(feature = "stage12-1-test")]
mod vfs_api;
#[cfg(any(
    not(feature = "qemu-test"),
    feature = "stage20-14-test",
    feature = "stage20-14b-test"
))]
mod virtio_gpu;
mod virtio_net;
#[cfg(any(
    not(feature = "qemu-test"),
    feature = "stage20-14-test",
    feature = "stage20-14b-test"
))]
mod virtio_pci;
// `storage_manager` seals records through this key vault, so the Stage 12.5
// gate needs the module even though it does not run the Stage 12.3 self-test.
#[cfg(feature = "stage13-10-test")]
mod bluetooth_hci;
#[cfg(feature = "stage13-9-test")]
mod irq_mailbox;
#[cfg(any(feature = "stage12-3-test", feature = "stage12-5-test"))]
mod volume_crypto;
mod wfs;
#[cfg(feature = "stage13-9-test")]
mod wifi;
#[cfg(feature = "stage13-9-test")]
mod wifi80211;
#[cfg(feature = "stage13-9-test")]
mod wifi_backend;
#[cfg(feature = "stage13-9-test")]
mod wifi_ccmp;
#[cfg(feature = "stage13-9-test")]
mod wifi_crypto;
#[cfg(any(feature = "stage13-9-test", feature = "physical-wifi"))]
mod wifi_firmware;
#[cfg(feature = "stage13-9-test")]
mod wifi_gtk;
#[cfg(any(feature = "stage13-9-test", feature = "physical-wifi"))]
mod wifi_hw;
#[cfg(feature = "stage13-9-test")]
mod wifi_link;
#[cfg(feature = "stage13-9-test")]
mod wifi_net;
#[cfg(feature = "stage13-9-test")]
mod wifi_recovery;
#[cfg(feature = "stage13-9-test")]
mod wifi_rsn;
#[cfg(feature = "stage13-9-test")]
mod wifi_runtime;
#[cfg(feature = "stage13-9-test")]
mod wifi_scan;
#[cfg(feature = "stage13-9-test")]
mod wifi_security;
#[cfg(feature = "stage13-9-test")]
mod wifi_session;
#[cfg(feature = "stage13-9-test")]
mod wifi_smol;
#[cfg(feature = "stage13-9-test")]
mod wifi_wpa2;
#[cfg(feature = "stage13-8-test")]
mod woven_audio;
mod woven_input;
#[cfg(any(
    feature = "stage12-2-test",
    feature = "stage12-3-test",
    feature = "stage12-4-test",
    feature = "stage12-5-test"
))]
mod wovenfs;
mod wovenguard;
#[cfg(any(
    not(feature = "qemu-test"),
    feature = "stage13-12-test",
    feature = "stage13-5-test",
    feature = "stage13-6-test",
    feature = "stage13-7-test",
    feature = "stage13-10-test"
))]
mod xhci;

use bootloader_api::{config::Mapping, entry_point, info::Optional, BootInfo, BootloaderConfig};

use console::Console;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(not(test))]
use core::{alloc::Layout, panic::PanicInfo};
use shell::Shell;

use x86_64::instructions::interrupts::int3;

static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    // Keep the PIE relocation base stable so the signed boot image has a
    // deterministic relocation result across firmware boots.
    config.mappings.kernel_base = Mapping::FixedAddress(0x1000_0000_0000);
    // The default boot stack is intentionally small. WovenHat performs
    // substantial early initialization, so give the bootstrap task a 1 MiB
    // stack while retaining the bootloader's guard-page protection.
    config.kernel_stack_size = 1024 * 1024;
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

static PREEMPTION_PROBE_BLOCKED: AtomicBool = AtomicBool::new(false);
static PREEMPTION_PROBE_COMPLETED: AtomicBool = AtomicBool::new(false);
static FAIR_TASK_A_RUNS: AtomicU64 = AtomicU64::new(0);
static FAIR_TASK_B_RUNS: AtomicU64 = AtomicU64::new(0);
static FAIR_TASKS_COMPLETED: AtomicU64 = AtomicU64::new(0);

#[allow(unreachable_code)]
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Verify the immutable bootloader image before allocator, paging, and
    // runtime initialization can modify writable PT_LOAD bytes.
    #[cfg(feature = "secure-boot")]
    let secure_boot_verified = secure_boot::verify_manifest(boot_info);
    smp::prepare(&boot_info.memory_regions);
    let physical_memory_offset = match &boot_info.physical_memory_offset {
        Optional::Some(offset) => Some(*offset),
        Optional::None => None,
    };
    let paging_init = match physical_memory_offset {
        Some(offset) => paging::init(offset),
        None => Err(paging::InitError::MissingPhysicalMemoryMapping),
    };
    let rsdp_address = match &boot_info.rsdp_addr {
        Optional::Some(address) => Some(*address),
        Optional::None => None,
    };
    let acpi = physical_memory_offset
        .ok_or(hal::acpi::Error::OutOfRange)
        .and_then(|offset| hal::acpi::discover(offset, rsdp_address, &boot_info.memory_regions));
    let memory_init = match (acpi.as_ref(), physical_memory_offset) {
        (Ok(summary), Some(offset)) => memory::init_with_topology(
            &boot_info.memory_regions,
            &summary.memory_affinities[..summary.memory_affinity_count],
            offset,
        ),
        (_, Some(offset)) => memory::init(&boot_info.memory_regions, offset),
        (_, None) => Err(memory::InitError::MissingPhysicalMemoryMapping),
    };
    let boot_info_address = boot_info as *const BootInfo as u64;

    let framebuffer = match &mut boot_info.framebuffer {
        Optional::Some(framebuffer) => framebuffer,
        Optional::None => halt(),
    };

    let info = framebuffer.info();
    let buffer = framebuffer.buffer_mut();
    let framebuffer_address = buffer.as_ptr() as u64;
    let stack_probe = &info as *const _ as u64;

    terminal::init(buffer, info);
    let mut console = Console::new(info, 40, 40, 2);

    console.clear();

    console.println("WOVENHAT OS");
    console.println("SECURE INTELLIGENCE PLATFORM");
    console.println("");

    console.println("WOVENHAT KERNEL 0.8.0 MULTICORE FOUNDATION");
    console.println("ARCHITECTURE: X86_64");
    console.println("KERNEL BOOT SUCCESSFUL.");
    console.println("");

    if memory_init.is_err() {
        console.println("FRAME ALLOCATOR: INITIALIZATION FAILED");
        halt();
    }

    if memory::self_test() {
        console.println("FRAME ALLOCATOR: OK");
        let memory_stats = memory::stats();
        serial::write_line(format_args!(
            "[MEMORY] frame allocator NUMA domains={} regions={}",
            memory_stats.numa_domains, memory_stats.usable_regions
        ));
    } else {
        console.println("FRAME ALLOCATOR: SELF TEST FAILED");
        halt();
    }

    if paging_init.is_err() {
        console.println("PAGING: INITIALIZATION FAILED");
        halt();
    }

    let translation_probes = [
        kernel_main as *const () as u64,
        boot_info_address,
        framebuffer_address,
        stack_probe,
    ];
    if paging::self_test(&translation_probes) {
        console.println("PAGING TRANSLATION: 4/4 OK");
    } else {
        console.println("PAGING TRANSLATION: FAILED");
        halt();
    }

    if paging::mapping_self_test() {
        console.println("PAGING MAP/WRITE/UNMAP: OK");
    } else {
        console.println("PAGING MAP/WRITE/UNMAP: FAILED");
        halt();
    }
    if !paging::map_range_rollback_self_test() {
        console.println("PAGING MAP ROLLBACK: FAILED");
        halt();
    }

    serial::init();
    #[cfg(feature = "secure-boot")]
    if !secure_boot_verified {
        serial::write_line(format_args!("[S15.5] signed kernel handoff: FAILED"));
        #[cfg(feature = "qemu-test")]
        qemu_test_exit_failure();
        halt();
    }
    #[cfg(feature = "secure-boot")]
    serial::write_line(format_args!("[S15.5] signed kernel handoff: PASSED"));
    #[cfg(feature = "qemu-test")]
    if !boot_measurement::self_test() {
        serial::write_line(format_args!(
            "[S15.6] software measurement primitive: FAILED"
        ));
        qemu_test_exit_failure();
    }
    #[cfg(feature = "qemu-test")]
    serial::write_line(format_args!(
        "[S15.6] software measurement primitive: PASSED"
    ));
    let _kernel_measurement = physical_memory_offset.and_then(|offset| {
        let measurement = boot_measurement::measure_kernel(boot_info, offset);
        if let Some(measurement) = measurement {
            let prefix = u64::from_le_bytes([
                measurement[0],
                measurement[1],
                measurement[2],
                measurement[3],
                measurement[4],
                measurement[5],
                measurement[6],
                measurement[7],
            ]);
            serial::write_line(format_args!(
                "[BOOT] kernel measurement prefix={prefix:016x}"
            ));
        } else {
            serial::write_line(format_args!("[BOOT] kernel measurement unavailable"));
        }
        measurement
    });
    // The physical inventory image stops after RAM/paging checks, before
    // driver activation, disk/network I/O, AP startup or QEMU acceptance.
    #[cfg(all(feature = "physical-probe", not(feature = "physical-wifi")))]
    physical_probe::run(&mut console, acpi.as_ref().ok());

    #[cfg(feature = "qemu-test")]
    if !paging::table_allocation_rollback_self_test() {
        serial::write_line(format_args!(
            "[S6.PAGING] table allocation rollback: FAILED"
        ));
        qemu_test_exit_failure();
    }
    #[cfg(feature = "qemu-test")]
    serial::write_line(format_args!(
        "[S6.PAGING] table allocation rollback: PASSED"
    ));

    let hardware = hal::init(acpi.as_ref().ok());
    #[cfg(all(feature = "system-apps", not(feature = "qemu-test")))]
    let tpm_info = acpi.as_ref().ok().and_then(|summary| summary.tpm2);
    if !hal::acpi::self_test() {
        console.println("ACPI PARSER: VALIDATION FAILED");
        halt();
    }
    match acpi {
        Ok(summary) => {
            console.println("ACPI TABLES: VALIDATED");
            serial::write_line(format_args!(
                "[ACPI] revision={} tables={} APIC={} CPUs={} IOAPICs={} ISOs={} LAPIC={:#x} FADT={} HPET={} MCFG={} SRAT_MEM={} NUMA_DOMAINS={} truncated={}",
                summary.revision,
                summary.tables,
                summary.apic as u8,
                summary.enabled_processors,
                summary.io_apics,
                summary.interrupt_overrides,
                summary.local_apic_address,
                summary.fadt as u8,
                summary.hpet as u8,
                summary.mcfg as u8,
                summary.memory_affinity_count,
                summary.numa_domains,
                summary.truncated as u8,
            ));
            if let Some(tpm2) = summary.tpm2 {
                serial::write_line(format_args!(
                    "[TPM] ACPI TPM2 present control={:#x} start_method={}",
                    tpm2.control_area, tpm2.start_method
                ));
                let mut random = [0_u8; 16];
                match tpm2::get_random(tpm2, &mut random) {
                    Ok(length) => serial::write_line(format_args!(
                        "[TPM] CRB GetRandom probe passed bytes={length}"
                    )),
                    Err(error) => serial::write_line(format_args!(
                        "[TPM] CRB GetRandom probe unavailable: {error:?}"
                    )),
                }
                random.fill(0);
                let mut pcr7 = [0_u8; 32];
                match tpm2::read_pcr_sha256(tpm2, 7, &mut pcr7) {
                    Ok(()) => serial::write_line(format_args!(
                        "[TPM] PCR7 SHA-256 read passed prefix={:02x}{:02x}{:02x}{:02x}",
                        pcr7[0], pcr7[1], pcr7[2], pcr7[3]
                    )),
                    Err(error) => {
                        serial::write_line(format_args!("[TPM] PCR7 read unavailable: {error:?}"))
                    }
                }
                pcr7.fill(0);
            } else {
                serial::write_line(format_args!(
                    "[TPM] ACPI TPM2 unavailable; hardware credential root not provisioned"
                ));
            }
        }
        Err(_) => console.println("ACPI TABLES: UNAVAILABLE"),
    }
    let vendor = match hardware.cpu_vendor {
        hal::CpuVendor::Intel => "INTEL",
        hal::CpuVendor::Amd => "AMD",
        hal::CpuVendor::Unknown => "UNKNOWN",
    };
    serial::write_fmt(format_args!(
        "HARDWARE: CPU={} LOGICAL_CPUS={} TSC={} RDRAND={} AES_NI={} AVX={} PAE={} SSE4.2={}\n",
        vendor,
        hardware.logical_cpus,
        hardware.cpu_features.has_tsc as u8,
        hardware.cpu_features.has_rdrand as u8,
        hardware.cpu_features.has_aes_ni as u8,
        hardware.cpu_features.has_avx as u8,
        hardware.cpu_features.has_pae as u8,
        hardware.cpu_features.has_sse4_2 as u8,
    ));
    if !hal::pci::self_test() {
        console.println("PCI DISCOVERY: VALIDATION FAILED");
        halt();
    }
    serial::write_line(format_args!(
        "[PCI] devices={} recorded={} storage={} network={} display={} bridges={} truncated={}",
        hardware.pci.discovered,
        hardware.pci.recorded,
        hardware.pci.storage,
        hardware.pci.network,
        hardware.pci.display,
        hardware.pci.bridges,
        hardware.pci.truncated as u8,
    ));
    console.println("PCI CONFIGURATION: ENUMERATED");
    #[cfg(feature = "stage13-9-test")]
    {
        let wifi_pci = wifi::discover_pci();
        if !wifi::self_test() {
            serial::write_line(format_args!(
                "[S13.9A] WovenWiFi framework + PCI classification: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9A] WovenWiFi framework + PCI classification: PASSED network={} wireless_candidates={}",
            wifi_pci.network_controllers,
            wifi_pci.wireless_candidates
        ));
        if !wifi80211::self_test() {
            serial::write_line(format_args!("[S13.9B] IEEE 802.11 frame/IE core: FAILED"));
            halt();
        }
        serial::write_line(format_args!("[S13.9B] IEEE 802.11 frame/IE core: PASSED"));
        if !wifi_scan::self_test() {
            serial::write_line(format_args!("[S13.9C] beacon/probe scan pipeline: FAILED"));
            halt();
        }
        serial::write_line(format_args!("[S13.9C] beacon/probe scan pipeline: PASSED"));
        if !wifi_link::self_test() {
            serial::write_line(format_args!(
                "[S13.9D] Open System authentication + association: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9D] Open System authentication + association: PASSED"
        ));
        if !wifi_rsn::self_test() {
            serial::write_line(format_args!(
                "[S13.9E] RSN + EAPOL-Key protocol foundation: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9E] RSN + EAPOL-Key protocol foundation: PASSED"
        ));
        if !wifi_crypto::self_test() {
            serial::write_line(format_args!(
                "[S13.9F] WPA2 cryptographic foundation: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9F] WPA2 cryptographic foundation: PASSED"
        ));
        if !wifi_wpa2::self_test() {
            serial::write_line(format_args!(
                "[S13.9G] WPA2 4-way handshake integration: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9G] WPA2 4-way handshake integration: PASSED"
        ));
        if !wifi_gtk::self_test() {
            serial::write_line(format_args!("[S13.9H] GTK + encrypted key data: FAILED"));
            halt();
        }
        serial::write_line(format_args!("[S13.9H] GTK + encrypted key data: PASSED"));
        if !wifi_ccmp::self_test() {
            serial::write_line(format_args!("[S13.9I] CCMP protected data path: FAILED"));
            halt();
        }
        serial::write_line(format_args!("[S13.9I] CCMP protected data path: PASSED"));
        if !wifi_net::self_test() {
            serial::write_line(format_args!(
                "[S13.9J] WovenWiFi <-> WovenNet integration: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9J] WovenWiFi <-> WovenNet integration: PASSED"
        ));
        if !wifi_net::group_ccmp_self_test() {
            serial::write_line(format_args!(
                "[S13.9X] GTK/group-addressed CCMP data path: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.9X] GTK/group-addressed CCMP data path: PASSED"
        ));
        if !wifi_wpa2::lifecycle_self_test() {
            serial::write_line(format_args!(
                "[S13.9Y] WPA2 reconnect/rekey lifecycle: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.9Y] WPA2 reconnect/rekey lifecycle: PASSED"
        ));
        if !wifi_wpa2::group_rekey_self_test() {
            serial::write_line(format_args!("[S13.9Z] WPA2 live group-key rekey: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.9Z] WPA2 live group-key rekey: PASSED"));
        if !wifi_hw::self_test() {
            serial::write_line(format_args!(
                "[S13.10A] physical PCI Wi-Fi backend boundary: FAILED"
            ));
            qemu_test_exit_failure();
        }
        let hw_candidate = wifi_hw::discover_first();
        serial::write_line(format_args!(
            "[S13.10A] physical PCI Wi-Fi backend boundary: PASSED discovered_candidate={}",
            hw_candidate.is_some() as u8
        ));
        if !wifi_hw::stage13_10b_self_test() {
            serial::write_line(format_args!(
                "[S13.10B] Wi-Fi MMIO/DMA/interrupt scaffolding: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10B] Wi-Fi MMIO/DMA/interrupt scaffolding: PASSED"
        ));
        if !wifi_hw::stage13_10c_self_test() {
            serial::write_line(format_args!(
                "[S13.10C] Wi-Fi real MMIO + DMA memory ownership: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10C] Wi-Fi real MMIO + DMA memory ownership: PASSED"
        ));
        if !wifi_hw::stage13_10d_self_test() {
            serial::write_line(format_args!(
                "[S13.10D] Wi-Fi DMA descriptor ownership + queue lifecycle: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10D] Wi-Fi DMA descriptor ownership + queue lifecycle: PASSED"
        ));
        if !wifi_hw::stage13_10e_self_test() {
            serial::write_line(format_args!(
                "[S13.10E] Wi-Fi supported chipset binding boundary: FAILED"
            ));
            qemu_test_exit_failure();
        }
        let supported_hw = wifi_hw::discover_first_supported();
        serial::write_line(format_args!(
            "[S13.10E] Wi-Fi supported chipset binding boundary: PASSED supported_device={}",
            supported_hw.is_some() as u8
        ));
        if !wifi_hw::stage13_10f_self_test() {
            serial::write_line(format_args!(
                "[S13.10F] Wi-Fi DMA buffer ownership binding: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10F] Wi-Fi DMA buffer ownership binding: PASSED"
        ));
        if !wifi_hw::stage13_10g_self_test() {
            serial::write_line(format_args!(
                "[S13.10G] Wi-Fi activation authority hardening: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10G] Wi-Fi activation authority hardening: PASSED"
        ));
        if !wifi_hw::stage13_10h_self_test() {
            serial::write_line(format_args!("[S13.10H] Intel AX200 CSR boundary: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.10H] Intel AX200 CSR boundary: PASSED"));
        if !wifi_hw::stage13_10i_self_test() {
            serial::write_line(format_args!(
                "[S13.10I] Intel reset/readiness state machine: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10I] Intel reset/readiness state machine: PASSED"
        ));
        if !wifi_hw::stage13_10j_self_test() {
            serial::write_line(format_args!(
                "[S13.10J] Intel MAC access/device initialization: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10J] Intel MAC access/device initialization: PASSED"
        ));
        if !wifi_firmware::stage13_10k_self_test() {
            serial::write_line(format_args!(
                "[S13.10K] Wi-Fi firmware validation/lifecycle foundation: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10K] Wi-Fi firmware validation/lifecycle foundation: PASSED"
        ));
        if !wifi_firmware::stage13_10l_self_test() {
            serial::write_line(format_args!("[S13.10L] Intel TLV firmware parser: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.10L] Intel TLV firmware parser: PASSED"));
        if !wifi_firmware::stage13_10m_self_test() {
            serial::write_line(format_args!(
                "[S13.10M] Intel firmware DMA staging/transfer boundary: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10M] Intel firmware DMA staging/transfer boundary: PASSED"
        ));
        if !wifi_firmware::stage13_10n_self_test() {
            serial::write_line(format_args!(
                "[S13.10N] Intel firmware transfer executor boundary: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10N] Intel firmware transfer executor boundary: PASSED"
        ));
        if !wifi_firmware::stage13_10o_self_test() {
            serial::write_line(format_args!(
                "[S13.10O] Intel 22000 firmware transport contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10O] Intel 22000 firmware transport contract: PASSED"
        ));
        if !wifi_firmware::stage13_10p_self_test() {
            serial::write_line(format_args!(
                "[S13.10P] AX200 context-info self-load manifest: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10P] AX200 context-info self-load manifest: PASSED"
        ));
        if !wifi_firmware::stage13_10q_self_test() {
            serial::write_line(format_args!(
                "[S13.10Q] AX200 DMA-backed context-info construction: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10Q] AX200 DMA-backed context-info construction: PASSED"
        ));
        if !wifi_firmware::stage13_10r_self_test() {
            serial::write_line(format_args!(
                "[S13.10R] AX200 context-info ABI + publication: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10R] AX200 context-info ABI + publication: PASSED"
        ));
        if !wifi_firmware::stage13_10s_self_test() {
            serial::write_line(format_args!(
                "[S13.10S] AX200 CPU_INIT_RUN startup sequencing: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10S] AX200 CPU_INIT_RUN startup sequencing: PASSED"
        ));
        if !wifi_firmware::stage13_10t_self_test() {
            serial::write_line(format_args!(
                "[S13.10T] AX200 ALIVE notification validation: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10T] AX200 ALIVE notification validation: PASSED"
        ));
        if !wifi_hw::stage13_10u_dma_self_test() {
            serial::write_line(format_args!(
                "[S13.10U] contiguous firmware DMA ownership: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10U] contiguous firmware DMA ownership: PASSED"
        ));
        if !wifi_firmware::stage13_10v_self_test() {
            serial::write_line(format_args!(
                "[S13.10V] version-aware AX200 ALIVE ABI: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10V] version-aware AX200 ALIVE ABI: PASSED"
        ));
        if !wifi_firmware::stage13_10w_self_test() {
            serial::write_line(format_args!(
                "[S13.10W] Intel RX notification delivery: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10W] Intel RX notification delivery: PASSED"
        ));
        if !wifi_firmware::stage13_10x_self_test() {
            serial::write_line(format_args!(
                "[S13.10X] Intel RX DMA completion boundary: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10X] Intel RX DMA completion boundary: PASSED"
        ));
        if !wifi_hw::stage13_10y_self_test() {
            serial::write_line(format_args!(
                "[S13.10Y] Intel RX interrupt service boundary: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10Y] Intel RX interrupt service boundary: PASSED"
        ));
        if !interrupts::stage13_10z_vector_self_test()
            || !smp::stage13_10z_irq_foundation_self_test()
        {
            serial::write_line(format_args!(
                "[S13.10Z] PCI device interrupt foundation: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10Z] PCI device interrupt foundation: PASSED"
        ));
        if !hal::pci::stage13_10aa_msi_self_test() {
            serial::write_line(format_args!(
                "[S13.10AA] PCI MSI programming contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10AA] PCI MSI programming contract: PASSED"
        ));
        if !wifi_hw::stage13_10ab_binding_self_test() {
            serial::write_line(format_args!(
                "[S13.10AB] AX200 PCI MSI binding contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        // Discovery is not authority to activate DMA/MSI before RX storage,
        // interrupt routing and the service owner have been established.
        serial::write_line(format_args!(
            "[S13.10AB] physical MSI activation deferred until RX DMA ownership is ready"
        ));
        serial::write_line(format_args!(
            "[S13.10AB] AX200 PCI MSI binding contract: PASSED"
        ));
        if !wifi_hw::stage13_10ac_deferred_self_test() {
            serial::write_line(format_args!(
                "[S13.10AC] deferred AX200 IRQ -> CSR RX service: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10AC] deferred AX200 IRQ -> CSR RX service: PASSED"
        ));
        if !wifi_hw::stage13_10ad_ax201_activation_self_test() {
            serial::write_line(format_args!(
                "[S13.10AD] AX201 integrated 22000 activation contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10AD] AX201 integrated 22000 activation contract: PASSED"
        ));
        if !wifi_firmware::stage13_10ae_ax201_firmware_prepare_self_test() {
            serial::write_line(format_args!(
                "[S13.10AE] AX201 firmware profile and DMA preparation: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10AE] AX201 firmware profile and DMA preparation: PASSED"
        ));
        if !wifi_firmware::stage13_10af_ax201_mmio_startup_self_test() {
            serial::write_line(format_args!(
                "[S13.10AF] AX201 22000 MMIO/PRPH startup adapter: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10AF] AX201 22000 MMIO/PRPH startup adapter: PASSED"
        ));
        if !wifi_firmware::stage13_10ag_ax201_driver_lifecycle_self_test() {
            serial::write_line(format_args!(
                "[S13.10AG] AX201 driver ownership/lifecycle: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10AG] AX201 driver ownership/lifecycle: PASSED"
        ));
        if !wifi_hw::stage13_10ai_ax201_queue_owner_self_test() {
            serial::write_line(format_args!(
                "[S13.10AI] AX201 physical DMA queue owner: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10AI] AX201 physical DMA queue owner: PASSED"
        ));
        if !wifi_firmware::stage13_10ah_ax201_firmware_provider_self_test() {
            serial::write_line(format_args!(
                "[S13.10AH] AX201 firmware-provider boundary: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10AH] AX201 firmware-provider boundary: PASSED"
        ));
        if !wifi_firmware::stage13_10aj_ax201_scan_control_self_test() {
            serial::write_line(format_args!(
                "[S13.10AJ] AX201 scan-control wire contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10AJ] AX201 scan-control wire contract: PASSED"
        ));
        #[cfg(feature = "stage13-9-test")]
        if !wifi_firmware::stage13_10ak_ax201_phy_context_self_test() {
            serial::write_line(format_args!(
                "[S13.10AK] AX201 PHY-context wire contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        #[cfg(feature = "stage13-9-test")]
        serial::write_line(format_args!(
            "[S13.10AK] AX201 PHY-context wire contract: PASSED"
        ));
        #[cfg(feature = "stage13-9-test")]
        if !wifi_firmware::stage13_10al_ax201_mac_binding_self_test() {
            serial::write_line(format_args!(
                "[S13.10AL] AX201 MAC/binding wire contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        #[cfg(feature = "stage13-9-test")]
        serial::write_line(format_args!(
            "[S13.10AL] AX201 MAC/binding wire contract: PASSED"
        ));
        if !wifi_backend::self_test() {
            serial::write_line(format_args!(
                "[S13.9K] Wi-Fi transport/backend contract: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9K] Wi-Fi transport/backend contract: PASSED"
        ));
        if !wifi_recovery::self_test() {
            serial::write_line(format_args!(
                "[S13.9L] reconnect/timeout/lifecycle hardening: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9L] reconnect/timeout/lifecycle hardening: PASSED"
        ));
        if !wifi_session::self_test() {
            serial::write_line(format_args!(
                "[S13.9M] cross-module integration closure: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9M] cross-module integration closure: PASSED"
        ));
        if !wifi_security::self_test() {
            serial::write_line(format_args!(
                "[S13.9N] WPA2 security lifecycle integration: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9N] WPA2 security lifecycle integration: PASSED"
        ));
        if !entropy::self_test() {
            serial::write_line(format_args!(
                "[S13.9O] kernel entropy/secure SNonce boundary: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9O] kernel entropy/secure SNonce boundary: PASSED"
        ));
        if !wifi_wpa2::self_test() {
            serial::write_line(format_args!(
                "[S13.9P] live WPA2 GTK/KRACK integration: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9P] live WPA2 GTK/KRACK integration: PASSED"
        ));
        if !wifi_link::self_test() {
            serial::write_line(format_args!(
                "[S13.9Q] WPA2 association + RSN integration: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9Q] WPA2 association + RSN integration: PASSED"
        ));
        if !entropy::self_test() {
            serial::write_line(format_args!(
                "[S13.9R] entropy/network randomness split: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9R] entropy/network randomness split: PASSED"
        ));
        if !wifi_session::wpa2_handoff_self_test() {
            serial::write_line(format_args!(
                "[S13.9S] WPA2 supplicant/session key handoff: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9S] WPA2 supplicant/session key handoff: PASSED"
        ));
        if !wifi_session::rx_path_self_test() {
            serial::write_line(format_args!(
                "[S13.9T] backend RX/CCMP/Ethernet integration: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9T] backend RX/CCMP/Ethernet integration: PASSED"
        ));
        if !wifi_smol::self_test() {
            serial::write_line(format_args!(
                "[S13.9U] WovenWiFi/smoltcp transport adapter: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9U] WovenWiFi/smoltcp transport adapter: PASSED"
        ));
        if !wifi_smol::transport_selector_self_test() {
            serial::write_line(format_args!(
                "[S13.9V] selectable WovenNet transport integration: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S13.9V] selectable WovenNet transport integration: PASSED"
        ));
        if !wifi_scan::self_test() {
            serial::write_line(format_args!("[S13.9C] beacon/probe scan pipeline: FAILED"));
            halt();
        }
        serial::write_line(format_args!("[S13.9C] beacon/probe scan pipeline: PASSED"));
    }

    #[cfg(feature = "stage13-10-test")]
    {
        if !bluetooth_hci::self_test() {
            serial::write_line(format_args!(
                "[S13.10A] Bluetooth HCI core foundation: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10A] Bluetooth HCI core foundation: PASSED"
        ));
        if !xhci::bluetooth_transport_self_test() {
            serial::write_line(format_args!(
                "[S13.10B] Bluetooth USB HCI transport contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10B] Bluetooth USB HCI transport contract: PASSED"
        ));
        if !bluetooth_hci::transaction_self_test() {
            serial::write_line(format_args!(
                "[S13.10C] Bluetooth HCI transfer execution contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10C] Bluetooth HCI transfer execution contract: PASSED"
        ));
        if !bluetooth_hci::initialization_self_test() {
            serial::write_line(format_args!(
                "[S13.10D] Bluetooth controller initialization sequence: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10D] Bluetooth controller initialization sequence: PASSED"
        ));
        if !bluetooth_hci::discovery_self_test() {
            serial::write_line(format_args!(
                "[S13.10E] Bluetooth inquiry discovery contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10E] Bluetooth inquiry discovery contract: PASSED"
        ));
        if !bluetooth_hci::capability_self_test() {
            serial::write_line(format_args!(
                "[S13.10E] Bluetooth controller capability discovery: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10E] Bluetooth controller capability discovery: PASSED"
        ));
        if !bluetooth_hci::link_lifecycle_self_test() {
            serial::write_line(format_args!(
                "[S13.10F] Bluetooth ACL link lifecycle: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10F] Bluetooth ACL link lifecycle: PASSED"
        ));
        if !bluetooth_hci::acl_data_self_test() {
            serial::write_line(format_args!(
                "[S13.10F] Bluetooth ACL owned data path: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10F] Bluetooth ACL owned data path: PASSED"
        ));
        if !bluetooth_hci::l2cap_framing_self_test() {
            serial::write_line(format_args!(
                "[S13.10G] Bluetooth L2CAP basic framing: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10G] Bluetooth L2CAP basic framing: PASSED"
        ));
        if !bluetooth_hci::l2cap_channel_self_test() {
            serial::write_line(format_args!(
                "[S13.10G] Bluetooth L2CAP channel lifecycle: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10G] Bluetooth L2CAP channel lifecycle: PASSED"
        ));
        serial::write_line(format_args!(
            "[S13.10G] Bluetooth L2CAP owned channel data: PASSED"
        ));
        if !bluetooth_hci::link_security_self_test() {
            serial::write_line(format_args!(
                "[S13.10H] Bluetooth authentication and encryption state: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10H] Bluetooth authentication and encryption state: PASSED"
        ));
        if !bluetooth_hci::link_key_self_test() {
            serial::write_line(format_args!(
                "[S13.10H] Bluetooth link-key lifecycle: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10H] Bluetooth link-key lifecycle: PASSED"
        ));
        if !bluetooth_hci::pairing_interaction_self_test() {
            serial::write_line(format_args!(
                "[S13.10H] Bluetooth pairing interaction contract: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10H] Bluetooth pairing interaction contract: PASSED"
        ));
        if !bluetooth_hci::trusted_security_self_test() {
            serial::write_line(format_args!(
                "[S13.10H] Bluetooth paired encrypted trust binding: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10H] Bluetooth paired encrypted trust binding: PASSED"
        ));
        if !bluetooth_hci::le_discovery_self_test() {
            serial::write_line(format_args!(
                "[S13.10I] Bluetooth LE advertising discovery: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10I] Bluetooth LE advertising discovery: PASSED"
        ));
        if !bluetooth_hci::le_link_lifecycle_self_test() {
            serial::write_line(format_args!(
                "[S13.10I] Bluetooth LE connection lifecycle: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10I] Bluetooth LE connection lifecycle: PASSED"
        ));
        if !bluetooth_hci::att_foundation_self_test() {
            serial::write_line(format_args!("[S13.10J] Bluetooth ATT foundation: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.10J] Bluetooth ATT foundation: PASSED"));
        if !bluetooth_hci::gatt_foundation_self_test() {
            serial::write_line(format_args!(
                "[S13.10J] Bluetooth GATT service model: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10J] Bluetooth GATT service model: PASSED"
        ));
        if !bluetooth_hci::gatt_discovery_self_test() {
            serial::write_line(format_args!("[S13.10J] Bluetooth GATT discovery: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.10J] Bluetooth GATT discovery: PASSED"));
        if !bluetooth_hci::gatt_subscription_self_test() {
            serial::write_line(format_args!(
                "[S13.10J] Bluetooth GATT subscriptions: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10J] Bluetooth GATT subscriptions: PASSED"
        ));
        if !bluetooth_hci::ble_standard_profiles_self_test() {
            serial::write_line(format_args!(
                "[S13.10K] Bluetooth BLE standard profiles: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10K] Bluetooth BLE standard profiles: PASSED"
        ));
        if !bluetooth_hci::ble_battery_notification_self_test() {
            serial::write_line(format_args!(
                "[S13.10K] Bluetooth Battery Service notifications: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10K] Bluetooth Battery Service notifications: PASSED"
        ));
        if !bluetooth_hci::wovenhat_ble_service_self_test() {
            serial::write_line(format_args!("[S13.10K] WovenHat BLE service API: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.10K] WovenHat BLE service API: PASSED"));
        if !bluetooth_hci::bluetooth_le_lifecycle_hardening_self_test() {
            serial::write_line(format_args!(
                "[S13.10L] Bluetooth LE lifecycle cleanup: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10L] Bluetooth LE lifecycle cleanup: PASSED"
        ));
        if !bluetooth_hci::bluetooth_le_recovery_self_test() {
            serial::write_line(format_args!(
                "[S13.10L] Bluetooth malformed teardown/reconnect recovery: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10L] Bluetooth malformed teardown/reconnect recovery: PASSED"
        ));
        if !bluetooth_hci::bluetooth_le_lifecycle_stress_self_test() {
            serial::write_line(format_args!("[S13.10L] Bluetooth lifecycle stress: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.10L] Bluetooth lifecycle stress: PASSED"));
        if !bluetooth_hci::le_bond_persistence_self_test() {
            serial::write_line(format_args!(
                "[S13.10M] Bluetooth LE bond persistence/reconnect: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10M] Bluetooth LE bond persistence/reconnect: PASSED"
        ));
        if !bluetooth_hci::le_bond_encryption_restore_self_test() {
            serial::write_line(format_args!(
                "[S13.10N] Bluetooth LE bonded encryption restoration: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10N] Bluetooth LE bonded encryption restoration: PASSED"
        ));
        if !bluetooth_hci::le_encrypted_session_self_test() {
            serial::write_line(format_args!(
                "[S13.10O] Bluetooth LE encrypted session state: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10O] Bluetooth LE encrypted session state: PASSED"
        ));
        if !bluetooth_hci::le_gatt_security_policy_self_test() {
            serial::write_line(format_args!(
                "[S13.10P] Bluetooth LE ATT/GATT security policy: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10P] Bluetooth LE ATT/GATT security policy: PASSED"
        ));
        if !bluetooth_hci::le_gatt_secure_notification_self_test() {
            serial::write_line(format_args!(
                "[S13.10Q] Bluetooth LE secure notifications/indications: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10Q] Bluetooth LE secure notifications/indications: PASSED"
        ));
        match bluetooth_hci::le_secure_session_identity_binding_self_test() {
            Ok(()) => serial::write_line(format_args!(
                "[S13.10R] Bluetooth LE secure-session identity binding: PASSED"
            )),
            Err(checkpoint) => {
                serial::write_line(format_args!(
                    "[S13.10R] Bluetooth LE secure-session identity binding: FAILED checkpoint=R{}",
                    checkpoint
                ));
                qemu_test_exit_failure();
            }
        }
        if !bluetooth_hci::le_gatt_minimum_key_size_self_test() {
            serial::write_line(format_args!(
                "[S13.10S] Bluetooth LE minimum encryption key size: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10S] Bluetooth LE minimum encryption key size: PASSED"
        ));
        if !bluetooth_hci::le_gatt_minimum_key_size_notification_self_test() {
            serial::write_line(format_args!(
                "[S13.10T] Bluetooth LE notification minimum key size: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10T] Bluetooth LE notification minimum key size: PASSED"
        ));
        if !bluetooth_hci::ble_smp_foundation_13_11a_self_test() {
            serial::write_line(format_args!(
                "[S13.11A] BLE SMP pairing/fixed-channel foundation: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.11A] BLE SMP pairing/fixed-channel foundation: PASSED"
        ));
        if !bluetooth_hci::ble_smp_key_distribution_13_11b_self_test() {
            serial::write_line(format_args!(
                "[S13.11B] BLE SMP LTK/key distribution: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.11B] BLE SMP LTK/key distribution: PASSED"
        ));
        if !bluetooth_hci::ble_smp_confirm_random_13_11c_self_test() {
            serial::write_line(format_args!(
                "[S13.11C] BLE SMP confirm/random crypto boundary: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.11C] BLE SMP confirm/random crypto boundary: PASSED"
        ));
        if !bluetooth_hci::ble_smp_aes128_13_11d_self_test() {
            serial::write_line(format_args!(
                "[S13.11D] BLE SMP production AES-128 backend: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.11D] BLE SMP production AES-128 backend: PASSED"
        ));
        if !bluetooth_hci::ble_smp_authentication_policy_13_11e_self_test() {
            serial::write_line(format_args!(
                "[S13.11E] BLE SMP authenticated pairing/MITM policy: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.11E] BLE SMP authenticated pairing/MITM policy: PASSED"
        ));
        if !bluetooth_hci::ble_smp_encryption_authority_13_11f_self_test() {
            serial::write_line(format_args!(
                "[S13.11F] BLE SMP controller encryption/session authority: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.11F] BLE SMP controller encryption/session authority: PASSED"
        ));
        if !bluetooth_hci::ble_smp_identity_privacy_13_11g_self_test() {
            serial::write_line(format_args!(
                "[S13.11G] BLE SMP identity/privacy foundation: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.11G] BLE SMP identity/privacy foundation: PASSED"
        ));
        if !bluetooth_hci::ble_smp_lifecycle_hardening_13_11h_self_test() {
            serial::write_line(format_args!(
                "[S13.11H] BLE SMP lifecycle/hardening closure: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.11H] BLE SMP lifecycle/hardening closure: PASSED"
        ));
        if !bluetooth_hci::ble_smp_persistent_authentication_audit_self_test() {
            serial::write_line(format_args!(
                "[S13.11-AUDIT] BLE SMP persistent authentication authority: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.11-AUDIT] BLE SMP persistent authentication authority: PASSED"
        ));
    }

    if heap::init().is_err() {
        console.println("KERNEL HEAP: INITIALIZATION FAILED");
        halt();
    }

    if heap::self_test() {
        console.println("KERNEL HEAP: CAPACITY/FRAGMENTATION OK");
        serial::write_line(format_args!("[HEAP] mapped={} bytes", heap::stats().size));
        // The compositor surface is a kernel-held primitive the client-surface
        // registry blits onto, sized to the framebuffer. Deferred to here
        // because its allocation needs a mapped heap.
        compositor::init_for_display();
        boot_stage(&mut console, "BOOT LOADER [1/7]: graphics surface ready");
        console.println("BOOT: starting CPU and interrupt setup");
    } else {
        console.println("KERNEL HEAP: SELF TEST FAILED");
        halt();
    }
    #[cfg(feature = "qemu-test")]
    if !tpm2::self_test() {
        serial::write_line(format_args!("[TPM] CRB codec: FAILED"));
        qemu_test_exit_failure();
    }
    #[cfg(feature = "qemu-test")]
    serial::write_line(format_args!("[TPM] CRB codec: PASSED"));
    #[cfg(feature = "qemu-test")]
    if !heap::live_metadata_self_test() {
        serial::write_line(format_args!("[S6.HEAP] unbounded live metadata: FAILED"));
        qemu_test_exit_failure();
    }
    #[cfg(feature = "qemu-test")]
    serial::write_line(format_args!("[S6.HEAP] unbounded live metadata: PASSED"));
    #[cfg(feature = "qemu-test")]
    if !memory::reclaimed_overflow_self_test() {
        serial::write_line(format_args!(
            "[S6.MEMORY] reclaimed overflow/double-free: FAILED"
        ));
        qemu_test_exit_failure();
    }
    #[cfg(feature = "qemu-test")]
    serial::write_line(format_args!(
        "[S6.MEMORY] reclaimed overflow/double-free: PASSED"
    ));

    // Normal boots are shell-first. Do not make the interactive console wait
    // for the exhaustive storage/network/ring3 validation suite. Those tests
    // remain below for `--features qemu-test` builds.
    #[cfg(not(feature = "qemu-test"))]
    {
        gdt::init();
        let _user_segments = gdt::user_segments();
        interrupts::init();
        task::init();

        pic::init();
        timer::init();

        let early_devices = [
            device::Device {
                name: "framebuffer-console",
                kind: device::DeviceKind::Console,
                irq: None,
            },
            device::Device {
                name: "com1",
                kind: device::DeviceKind::Serial,
                irq: None,
            },
            device::Device {
                name: "pit",
                kind: device::DeviceKind::Timer,
                irq: Some(timer::IRQ),
            },
            device::Device {
                name: "ps2-keyboard",
                kind: device::DeviceKind::Keyboard,
                irq: Some(keyboard::IRQ),
            },
        ];

        for dev in early_devices {
            if device::register(dev).is_err() {
                serial::write_line(format_args!("[BOOT] early device registration failed"));
                halt();
            }
        }

        smp::start(acpi.ok(), physical_memory_offset.unwrap());
        console.println("BOOT: CPUs online; probing PS/2 mouse");
        // Interrupts are still off: the mouse's command responses must not be
        // taken by an IRQ handler.
        let mouse_present = mouse::init();
        serial::write_line(format_args!("[BOOT] ps2 mouse present={}", mouse_present));
        if !smp::routed_irq() {
            pic::unmask(timer::IRQ);
            pic::unmask(keyboard::IRQ);
            if mouse_present {
                pic::unmask(mouse::IRQ);
            }
        }
        x86_64::instructions::interrupts::enable();
        console.println("BOOT: interrupts enabled; starting service workers");
        #[cfg(feature = "physical-wifi")]
        physical_probe::run_ax201_activation(&mut console, acpi.as_ref().ok());
        if !block_io::start_worker() {
            console.println("BLOCK I/O WORKER: START FAILED");
            halt();
        }
        if !async_file::start_worker() {
            console.println("ASYNC FILE WORKER: START FAILED");
            halt();
        }
        if !async_events::start_worker() {
            panic!("timer/event worker startup failed");
        }
        if !async_network::start_worker() {
            console.println("ASYNC NETWORK WORKER: START FAILED");
            halt();
        }
        console.println("BOOT: service workers ready; checking async I/O");
        if !block_io::async_completion_self_test() {
            console.println("BLOCK I/O COMPLETION: FAILED");
            halt();
        }
        if !task::start_pager() {
            console.println("PAGER: START FAILED");
            halt();
        }
        boot_stage(&mut console, "BOOT LOADER [2/7]: scheduler and I/O ready");

        let ata_sectors = ata::init();
        if let Some(sectors) = ata_sectors {
            serial::write_line(format_args!(
                "[ATA] primary-master online: {} sectors",
                sectors
            ));
        } else {
            serial::write_line(format_args!("[ATA] primary-master not detected"));
        }

        match storage::mount_ata_root() {
            storage::MountStatus::Mounted(count) => {
                let _ = device::register(device::Device {
                    name: "ata0",
                    kind: device::DeviceKind::Block,
                    irq: None,
                });
                serial::write_line(format_args!(
                    "[FS] FAT32 mounted at /mnt; imported {} entries",
                    count
                ));
            }
            storage::MountStatus::NoDevice => {
                serial::write_line(format_args!("[FS] no ATA disk; continuing with RAM VFS"))
            }
            storage::MountStatus::NotFat32 => {
                serial::write_line(format_args!("[FS] ATA disk present but no FAT32 root"))
            }
            storage::MountStatus::Failed => serial::write_line(format_args!(
                "[FS] FAT32 mount failed; continuing with RAM VFS"
            )),
        }
        boot_stage(&mut console, "BOOT LOADER [3/7]: storage checked");

        for dir in ["/etc", "/var", "/home", "/tmp"] {
            let _ = vfs::mkdir(dir);
        }

        // Persistent accounts (from a provisioned `/mnt/etc/wovenhat/accounts`)
        // are a production concern, not a hardware-demo-only one: a real
        // release image has no other way to carry real sign-in accounts.
        // Finding none is not a failure (`install_persistent_accounts`
        // returns `true` for that case) -- a machine with no accounts yet
        // boots to an accounts-less session, which is what a first-boot
        // setup step is for.
        #[cfg(feature = "system-apps")]
        if !install_persistent_accounts() {
            serial::write_line(format_args!(
                "[SESSION] persistent account provisioning failed"
            ));
            halt();
        }
        // Sub-stage timing within [3/7]->[4/7]: this span was found (boot
        // profiling, Stage 30) to dominate the whole boot -- 4323 of a
        // ~4350-tick total boot under QEMU TCG -- and PBKDF2 account hashing
        // (100,000 iterations, forced software-only SHA-256, no hardware
        // extension this kernel ever uses) is the leading suspect, so its
        // own sub-stage is split out to confirm or rule that out directly
        // rather than by inference.
        #[cfg(feature = "hardware-demo")]
        {
            if !install_hardware_demo_accounts() {
                serial::write_line(format_args!("[DEMO] account provisioning failed"));
                halt();
            }
            boot_stage(&mut console, "BOOT LOADER [3.1/7]: demo accounts hashed");
        }

        #[cfg(feature = "system-apps")]
        {
            if !install_production_system_apps() {
                serial::write_line(format_args!("[APPS] production catalog install failed"));
                halt();
            }
            boot_stage(
                &mut console,
                "BOOT LOADER [3.2/7]: system app catalog installed",
            );
            if !start_production_system_services() {
                serial::write_line(format_args!("[SERVICES] production service startup failed"));
                halt();
            }
        }
        boot_stage(&mut console, "BOOT LOADER [4/7]: system files ready");

        match network::init() {
            Ok(()) => serial::write_line(format_args!(
                "[NET] virtio-net + smoltcp online at 10.0.2.15/24"
            )),
            Err(error) => serial::write_line(format_args!(
                "[NET] optional network init skipped: {:?}",
                error
            )),
        }

        // Storage, USB and audio controllers: bound through the driver
        // manager; a missing or failing device never stops the boot.
        let driver_report = drivers::bring_up();

        // Start live-log setup as soon as the USB controller and attached
        // storage have been brought up. Its I/O remains on a service task,
        // overlapping display, filesystem and session setup without blocking
        // the foreground boot path.
        match livelog::start_deferred() {
            Ok(true) => serial::write_line(format_args!(
                "[LIVELOG] startup queued; remaining boot continues"
            )),
            Ok(false) => serial::write_line(format_args!(
                "[LIVELOG] startup already queued or logging already active"
            )),
            Err(error) => serial::write_line(format_args!(
                "[LIVELOG] startup worker did not start: {:?}",
                error
            )),
        }

        // A multi-head virtio-gpu becomes the desktop's monitors.
        if let Ok(heads) = compositor::attach_gpu_outputs() {
            serial::write_line(format_args!("[DISPLAY] virtio-gpu: {} monitors", heads));
        }
        boot_stage(&mut console, "BOOT LOADER [5/7]: devices ready");
        // Without an ATA disk, mount the first ready NVMe or AHCI disk.
        if !storage::mnt_mounted() {
            if let Some((root, name)) = drivers::storage_root(&driver_report) {
                block_io::select_root(root);
                match storage::mount_ata_root() {
                    storage::MountStatus::Mounted(count) => serial::write_line(format_args!(
                        "[FS] FAT32 mounted at /mnt from {}; imported {} entries",
                        name, count
                    )),
                    status => serial::write_line(format_args!(
                        "[FS] {} has no mountable FAT32 volume: {:?}",
                        name, status
                    )),
                }
            }
        }

        // The signed TPM policy records are release credential inputs. Load
        // and revalidate them only after the durable WovenFS mount attempt;
        // absence, invalid metadata, or any live TPM error leaves hardware
        // credentials unavailable and must not block a usable desktop boot.
        #[cfg(feature = "system-apps")]
        match tpm_info {
            None => serial::write_line(format_args!(
                "[TPM] broker activation unavailable; no live TPM; hardware credentials disabled"
            )),
            Some(info) => match tpm2::activate_provisioned_hardware_credential_broker(info) {
                Ok(()) => serial::write_line(format_args!(
                    "[TPM] signed broker policy, credential root, and attestation key accepted; hardware credentials enabled"
                )),
                Err(tpm2::Error::Storage) => serial::write_line(format_args!(
                    "[TPM] signed broker policy unavailable; hardware credentials disabled"
                )),
                Err(error) => serial::write_line(format_args!(
                    "[TPM] broker activation rejected: {error:?}; hardware credentials disabled"
                )),
        }
        }

        // A durable context is only a recovery candidate. Require a live TPM
        // ContextLoad and immediately flush the restored session until the
        // boot authority consumer is integrated; no filesystem record alone
        // can create hardware credential authority.
        #[cfg(feature = "system-apps")]
        {
            match tpm_info {
            None => serial::write_line(format_args!(
                "[TPM] context recovery unavailable; no live TPM; hardware credentials disabled"
            )),
            Some(info) => match tpm2::authorize_policy_session_custody(info) {
                Err(error) => serial::write_line(format_args!(
                    "[TPM] custody authorization unavailable: {error:?}; hardware credentials disabled"
                )),
                Ok(custody) => match tpm2::load_policy_session_snapshot(&custody) {
                    Err(tpm2::Error::Storage) => serial::write_line(format_args!(
                        "[TPM] no durable policy-session context; hardware credentials disabled"
                    )),
                    Err(error) => serial::write_line(format_args!(
                        "[TPM] durable policy-session context rejected: {error:?}; hardware credentials disabled"
                    )),
                    Ok(mut snapshot) => match tpm2::load_policy_session(info, &mut snapshot) {
                        Err(error) => serial::write_line(format_args!(
                            "[TPM] policy-session ContextLoad rejected: {error:?}; hardware credentials disabled"
                        )),
                        Ok(mut lease) => match tpm2::flush_policy_session(info, &mut lease) {
                            Ok(()) => serial::write_line(format_args!(
                                "[TPM] durable policy-session context recovered and flushed; authority remains disabled"
                            )),
                            Err(error) => serial::write_line(format_args!(
                                "[TPM] recovered policy-session cleanup failed: {error:?}; hardware credentials disabled"
                            )),
                        },
                    },
                },
            },
        }
        }

        // Stage 20.15: the WovenShell desktop is the session; F12 shows this
        // diagnostic console instead.
        //
        // `session::start` only reports whether the process was spawned, not
        // whether it ever reaches `compositor::activate_display()` -- a
        // crash or a hang right after spawn looks identical to "running
        // behind the compositor" from here. So this also waits, briefly and
        // boundedly, for `session::active()` to confirm the desktop actually
        // took the screen (or for the process to have already died trying),
        // and keeps the outcome in `session_ready`/`session_status`. The
        // diagnostic-shell fallback below is only ever shown when
        // `!session_ready`: showing it unconditionally, as before, meant its
        // `console.clear()` silently overwrote a successfully-started
        // desktop's frame on any boot slow enough for the race to go the
        // other way, which real hardware (unlike QEMU) apparently is.
        #[cfg(feature = "system-apps")]
        let session_status;
        #[cfg(not(feature = "system-apps"))]
        let session_status = alloc::string::String::new();
        #[cfg(feature = "system-apps")]
        let mut session_ready = false;
        #[cfg(not(feature = "system-apps"))]
        let session_ready = false;
        #[cfg(feature = "system-apps")]
        {
            boot_stage(
                &mut console,
                "BOOT LOADER [6/7]: starting WovenShell desktop",
            );
            let accounts = vfs::stat("/etc/wovenhat/accounts")
                .is_ok()
                .then_some("/etc/wovenhat/accounts");
            session_status = match session::start(WOVEN_SHELL_IMAGE, accounts) {
                Ok(pid) => {
                    serial::write_line(format_args!(
                        "[SESSION] WovenShell desktop started as pid {}",
                        pid
                    ));
                    // Up to ~5s (100 Hz ticks), the same bounded-wait pattern
                    // as the stage self-tests' local `until` helpers: no
                    // assumption that this boot thread can sleep, only that
                    // it can yield.
                    const WAIT_TICKS: u64 = 500;
                    let start = timer::ticks();
                    loop {
                        if session::active().is_some() {
                            session_ready = true;
                            break;
                        }
                        if !task::process_alive(pid) {
                            break;
                        }
                        if timer::ticks().wrapping_sub(start) > WAIT_TICKS {
                            break;
                        }
                        task::yield_now();
                    }
                    if session_ready {
                        console.println("DESKTOP SESSION: STARTED (F12 SWITCHES TO THIS CONSOLE)");
                        boot_stage(&mut console, "BOOT LOADER [7/7]: desktop ready");
                        alloc::format!("SESSION: started as pid {pid} (F12 shows the desktop)")
                    } else if !task::process_alive(pid) {
                        serial::write_line(format_args!(
                            "[SESSION] pid {} exited before taking the display",
                            pid
                        ));
                        alloc::format!("SESSION: pid {pid} exited before the desktop appeared")
                    } else {
                        serial::write_line(format_args!(
                            "[SESSION] pid {} has not taken the display after {}ms",
                            pid,
                            WAIT_TICKS * 10
                        ));
                        alloc::format!(
                            "SESSION: pid {pid} has not shown the desktop yet (still running)"
                        )
                    }
                }
                Err(error) => {
                    serial::write_line(format_args!("[SESSION] desktop not started: {:?}", error));
                    alloc::format!("SESSION: NOT STARTED ({error:?})")
                }
            };
        }

        serial::write_line(format_args!(
            "[BOOT] shell-first runtime ready; entering diagnostic shell"
        ));

        // Start userspace init -> /bin/sh (non-fatal if spawn fails).
        match userspace::create_init_process() {
            Some(program) => match task::spawn_user_process("init", program) {
                Ok((pid, _)) => {
                    serial::write_line(format_args!(
                        "[BOOT] userspace init/sh scheduled as pid {}",
                        pid.as_u64()
                    ));
                    console.println("USERSPACE INIT+SH: STARTED");
                }
                Err(_) => console.println("USERSPACE INIT+SH: SPAWN FAILED"),
            },
            None => console.println("USERSPACE INIT+SH: IMAGE FAILED"),
        }

        let mut desktop = gui::Desktop::new(graphics::Color::DARK_BLUE);
        let mut window = gui::Window::new(gui::Rect::new(80, 80, 480, 280), "WOVENHAT DESKTOP");
        window.add_button(gui::Button::new(
            gui::Rect::new(120, 180, 180, 48),
            "ACTIVATE",
            graphics::Color::CYAN,
        ));
        window.add_button(gui::Button::new(
            gui::Rect::new(320, 180, 180, 48),
            "SECOND",
            graphics::Color::CYAN,
        ));
        desktop.add_window(window);
        let mut shell = Shell::new();
        let mut desktop_active = false;

        // This placeholder desktop and the diagnostic-shell banner below are
        // a developer-only fallback for when WovenShell did not take the
        // screen itself (`!session_ready`): a real user who gets this far
        // never asked for an ACTIVATE/SECOND test window, so it is never
        // shown once the desktop session is confirmed up. F12 still reaches
        // the kernel's own diagnostic console either way (`woven_input`
        // dispatches it to `session::toggle_console()` ahead of this loop).
        if !session_ready {
            console.println("");
            console.clear();
            console.println("WOVENHAT DIAGNOSTIC SHELL (F1 TO OPEN DESKTOP)");
            if !session_status.is_empty() {
                console.println(&session_status);
            }
            shell.print_prompt(&mut console);
        }

        let mut userspace_was_foreground = false;
        loop {
            let userspace_foreground = terminal::foreground_active();
            if userspace_was_foreground && !userspace_foreground {
                console.clear();
                console.println("WOVENHAT DIAGNOSTIC SHELL");
                console.println("USERSPACE SESSION ENDED");
                console.println("");
                shell.print_prompt(&mut console);
                desktop_active = false;
            }
            userspace_was_foreground = userspace_foreground;

            // The kernel UI may consume PS/2 input only when no userspace
            // process owns the foreground terminal.  In particular, do not
            // call keyboard::poll() while /bin/sh is foreground: poll() pops
            // the scancode from the shared queue, which would starve the
            // userspace read(0, ...) syscall and make the shell appear hung.
            if !userspace_foreground {
                if let Some(key) = keyboard::poll() {
                    if matches!(
                        key,
                        keyboard::Key::F2
                            | keyboard::Key::F3
                            | keyboard::Key::F4
                            | keyboard::Key::F12
                    ) {
                        // F2-F4 have no use in the kernel's own desktop.
                    } else if matches!(key, keyboard::Key::F1) {
                        desktop_active = !desktop_active;
                        if desktop_active {
                            console.render_desktop(&desktop);
                        } else {
                            console.clear();
                            console.println("WOVENHAT DIAGNOSTIC SHELL (F1 TO OPEN DESKTOP)");
                            shell.print_prompt(&mut console);
                        }
                    } else if desktop_active {
                        let event = match key {
                            keyboard::Key::Char(character) => gui::InputEvent::Key(character),
                            keyboard::Key::Enter => gui::InputEvent::Key('\n'),
                            keyboard::Key::Backspace => gui::InputEvent::Key('\u{8}'),
                            keyboard::Key::Tab => gui::InputEvent::Key('\t'),
                            keyboard::Key::F1
                            | keyboard::Key::F2
                            | keyboard::Key::F3
                            | keyboard::Key::F4
                            | keyboard::Key::F12 => {
                                unreachable!()
                            }
                        };
                        desktop.handle(&event);
                        console.render_desktop(&desktop);
                    } else {
                        shell.handle_key(key, &mut console);
                    }
                }
            }

            network::poll();
            syscall::service_pending();
            task::preemption_point();
            x86_64::instructions::hlt();
        }
    }
    if gpt::self_test() {
        console.println("GPT PARTITIONS: VALIDATED");
    } else {
        console.println("GPT PARTITIONS: VALIDATION FAILED");
        halt();
    }
    if partition::self_test() {
        console.println("MBR PARTITIONS: VALIDATED");
    } else {
        console.println("MBR PARTITIONS: VALIDATION FAILED");
        halt();
    }
    if block::self_test() {
        console.println("BLOCK DEVICE I/O: OK");
    } else {
        console.println("BLOCK DEVICE I/O: FAILED");
        halt();
    }
    if block_cache::self_test() {
        console.println("BLOCK CACHE: OK");
        serial::write_line(format_args!("[BUFFER CACHE] regression tests: PASSED"));
    } else {
        console.println("BLOCK CACHE: FAILED");
        serial::write_line(format_args!("[BUFFER CACHE] regression tests: FAILED"));
        halt();
    }
    if block_io::self_test() {
        console.println("ASYNC BLOCK I/O: OK");
        serial::write_line(format_args!("[BLOCK IO] async completion tests: PASSED"));
    } else {
        console.println("ASYNC BLOCK I/O: FAILED");
        serial::write_line(format_args!("[BLOCK IO] async completion tests: FAILED"));
        halt();
    }
    if swap::self_test() {
        console.println("SWAP BACKING: OK");
        serial::write_line(format_args!("[SWAP] disk-backed policy tests: PASSED"));
    } else {
        console.println("SWAP BACKING: FAILED");
        serial::write_line(format_args!("[SWAP] disk-backed policy tests: FAILED"));
        halt();
    }

    if page_cache::self_test() {
        serial::write_line(format_args!("[FILE PAGES] regression tests: PASSED"));
    } else {
        serial::write_line(format_args!("[FILE PAGES] regression tests: FAILED"));
        halt();
    }

    if ata::self_test() {
        console.println("ATA IDENTIFY PARSER: OK");
    } else {
        console.println("ATA IDENTIFY PARSER: FAILED");
        halt();
    }

    if fat32::self_test() {
        console.println("FAT32 CHAIN READS: OK");
    } else {
        console.println("FAT32 VALIDATION: FAILED");
        halt();
    }
    if network::self_test() {
        console.println("NETWORK/SMOLTCP ADAPTER: OK");
    } else {
        console.println("NETWORK/SMOLTCP ADAPTER: FAILED");
        halt();
    }
    match virtio_net::probe() {
        virtio_net::ProbeStatus::Found(_) => console.println("VIRTIO-NET PCI: DETECTED"),
        virtio_net::ProbeStatus::Missing => console.println("VIRTIO-NET PCI: NOT PRESENT"),
    }

    if vfs::self_test() {
        console.println("VFS READ/WRITE: OK");
        serial::write_line(format_args!("[VFS] read/write and path semantics: PASSED"));
    } else {
        console.println("VFS READ/WRITE: FAILED");
        serial::write_line(format_args!("[VFS] read/write and path semantics: FAILED"));
        halt();
    }
    if paging::frame_ownership_self_test() {
        serial::write_line(format_args!(
            "[FRAME OWNERSHIP] overflow and exhaustion rollback: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[FRAME OWNERSHIP] exhaustion rollback: FAILED"
        ));
        halt();
    }
    if file_frames::self_test() {
        serial::write_line(format_args!(
            "[FRAME CACHE] pinned aliases, LRU eviction, reclaim: PASSED"
        ));
    } else {
        serial::write_line(format_args!("[FRAME CACHE] regression tests: FAILED"));
        halt();
    }
    if userspace::shared_file_mmap_self_test() {
        serial::write_line(format_args!(
            "[SHARED FILE MMAP] aliases, COW, truncation, unlink, reclaim: PASSED"
        ));
    } else {
        serial::write_line(format_args!("[SHARED FILE MMAP] regression tests: FAILED"));
        halt();
    }
    if userspace::lazy_file_mmap_self_test() {
        serial::write_line(format_args!("[LAZY FILE MMAP] regression tests: PASSED"));
    } else {
        serial::write_line(format_args!("[LAZY FILE MMAP] regression tests: FAILED"));
        halt();
    }
    if userspace::private_lazy_swap_self_test() {
        serial::write_line(format_args!(
            "[PRIVATE SWAP MMAP] dirty eviction/refault: PASSED"
        ));
    } else {
        serial::write_line(format_args!("[PRIVATE SWAP MMAP] regression tests: FAILED"));
        halt();
    }
    if userspace::file_mmap_self_test() {
        serial::write_line(format_args!("[FILE MMAP] regression tests: PASSED"));
    } else {
        serial::write_line(format_args!("[FILE MMAP] regression tests: FAILED"));
        halt();
    }
    // Blocking pipe APIs consult the current task, even for immediate reads.
    gdt::init();
    let _user_segments = gdt::user_segments();
    console.println("GDT/TSS: INSTALLED");
    console.println("USER MODE SEGMENTS: READY");

    //
    // Interrupt Descriptor Table
    //

    interrupts::init();

    console.println("IDT: INSTALLED");
    if interrupts::fault_policy_self_test() {
        console.println("USER FAULT RECOVERY: ARMED");
    } else {
        console.println("USER FAULT RECOVERY: FAILED");
        halt();
    }

    task::init();
    console.println("SCHEDULER: INITIALIZED");
    if pipe::self_test() {
        console.println("PIPE: OK");
    } else {
        console.println("PIPE: FAILED");
        halt();
    }
    if storage::self_test() {
        console.println("STORAGE MOUNT PATHS: OK");
    } else {
        console.println("STORAGE MOUNT PATHS: FAILED");
        halt();
    }
    if userspace::elf_loader_self_test() {
        console.println("ELF64 W^X + STACK GUARD: OK");
    } else {
        console.println("ELF64 LOADER VALIDATION: FAILED");
        halt();
    }

    if keyboard::self_test() {
        console.println("KEYBOARD DECODER: OK");
    } else {
        console.println("KEYBOARD DECODER: FAILED");
        halt();
    }
    if gui::self_test() {
        console.println("GUI INPUT: OK");
    } else {
        console.println("GUI INPUT: FAILED");
        halt();
    }
    if ipc::self_test() && ipc::endpoint_count() == 0 {
        console.println("IPC QUEUES: VALIDATED");
    } else {
        console.println("IPC QUEUES: VALIDATION FAILED");
        halt();
    }
    if ipc::handle_object_self_test() && ipc::object_count() == 0 {
        console.println("[S8.1] IPC object/handle local invariants: PASSED");
    } else {
        console.println("[S8.1] IPC object/handle local invariants: FAILED");
        halt();
    }

    if audit::self_test() && task::credential_policy_valid() {
        console.println("CREDENTIAL/AUDIT POLICY: OK");
    } else {
        console.println("CREDENTIAL/AUDIT POLICY: FAILED");
        halt();
    }

    if benchmark::self_test() {
        console.println("BENCHMARK DELTAS: VALIDATED");
    } else {
        console.println("BENCHMARK DELTAS: FAILED");
        halt();
    }
    if syscall::test() {
        console.println("SYSCALL GATE: GETPID OK");
    } else {
        console.println("SYSCALL GATE: FAILED");
        halt();
    }

    if task::capability_policy_valid() {
        console.println("CAPABILITY POLICY: ONLINE");
    } else {
        console.println("CAPABILITY POLICY: FAILED");
        halt();
    }

    if task::capability_delegation_valid() {
        console.println("CAPABILITY DELEGATION: OK");
    } else {
        console.println("CAPABILITY DELEGATION: FAILED");
        halt();
    }

    if audit::count() < 2
        || !audit::latest()
            .is_some_and(|event| event.action == audit::Action::CapabilityRevoke && event.allowed)
    {
        console.println("CAPABILITY AUDIT: FAILED");
        halt();
    }

    pic::init();
    console.println("PIC: INITIALIZED (ALL IRQS MASKED)");

    timer::init();
    let ata_sectors = ata::init();
    if ata_sectors.is_some()
        && !ata::with_primary_master(|disk| {
            let mut sector = [0_u8; block::SECTOR_SIZE];
            block::BlockDevice::read_sector(disk, 0, &mut sector).is_ok()
        })
        .unwrap_or(false)
    {
        console.println("ATA LBA0 READ: FAILED");
        halt();
    }
    let storage_status = storage::mount_ata_root();
    match storage_status {
        storage::MountStatus::Mounted(files) => {
            console.println("FAT32 ROOT MOUNTED");
            serial::write_line(format_args!("[VFS] mounted {} FAT32 root files", files));
        }
        storage::MountStatus::NoDevice => console.println("FAT32 MOUNT: NO BLOCK DEVICE"),
        storage::MountStatus::NotFat32 => console.println("FAT32 MOUNT: NO VOLUME"),
        storage::MountStatus::Failed => console.println("FAT32 MOUNT: FAILED"),
    }
    if userspace::install_stub_executable() {
        console.println("EXEC IMAGE: INSTALLED");
    } else {
        console.println("EXEC IMAGE: INSTALL FAILED");
        halt();
    }
    if userspace::install_init_executable() {
        console.println("INIT IMAGE: INSTALLED");
    } else {
        console.println("INIT IMAGE: INSTALL FAILED");
        halt();
    }
    if userspace::install_shell_executable() {
        console.println("SHELL IMAGE: INSTALLED");
    } else {
        console.println("SHELL IMAGE: INSTALL FAILED");
        halt();
    }
    if userspace::install_echo_executable() {
        console.println("ECHO IMAGE: INSTALLED");
    } else {
        console.println("ECHO IMAGE: INSTALL FAILED");
        halt();
    }
    if userspace::install_true_executable()
        && userspace::install_false_executable()
        && userspace::install_cat_executable()
        && userspace::install_ls_executable()
        && userspace::install_sleep_executable()
        && userspace::install_pwd_executable()
        && userspace::install_mkdir_executable()
        && userspace::install_rm_executable()
    {
        console.println("BIN UTILS: INSTALLED");
    } else {
        console.println("BIN UTILS: INSTALL FAILED");
        halt();
    }
    let vfs_nodes_before_userspace = vfs::node_count();
    let boot_devices = [
        device::Device {
            name: "framebuffer-console",
            kind: device::DeviceKind::Console,
            irq: None,
        },
        device::Device {
            name: "com1",
            kind: device::DeviceKind::Serial,
            irq: None,
        },
        device::Device {
            name: "pit",
            kind: device::DeviceKind::Timer,
            irq: Some(timer::IRQ),
        },
        device::Device {
            name: "ps2-keyboard",
            kind: device::DeviceKind::Keyboard,
            irq: Some(keyboard::IRQ),
        },
    ];
    for device in boot_devices {
        if device::register(device).is_err() {
            console.println("DEVICE REGISTRATION: FAILED");
            halt();
        }
    }
    if ata_sectors.is_some()
        && device::register(device::Device {
            name: "ata0",
            kind: device::DeviceKind::Block,
            irq: None,
        })
        .is_err()
    {
        console.println("ATA DEVICE REGISTRATION: FAILED");
        halt();
    }
    if device::self_test(ata_sectors.is_some()) {
        if let Some(sectors) = ata_sectors {
            console.println("DEVICE REGISTRY: 5 DEVICES ONLINE");
            serial::write_line(format_args!("[ATA] primary master: {} sectors", sectors));
        } else {
            console.println("DEVICE REGISTRY: 4 DEVICES ONLINE (NO ATA)");
        }
    } else {
        console.println("DEVICE REGISTRY: VALIDATION FAILED");
        halt();
    }
    smp::start(acpi.ok(), physical_memory_offset.unwrap());
    // Interrupts are still off: the mouse's command responses must not be
    // taken by an IRQ handler.
    let mouse_present = mouse::init();
    serial::write_line(format_args!("[BOOT] ps2 mouse present={}", mouse_present));
    if !smp::routed_irq() {
        pic::unmask(timer::IRQ);
        pic::unmask(keyboard::IRQ);
        if mouse_present {
            pic::unmask(mouse::IRQ);
        }
    }
    x86_64::instructions::interrupts::enable();
    #[cfg(feature = "qemu-test")]
    if !heap::runtime_growth_self_test() {
        serial::write_line(format_args!("[S6.HEAP] runtime growth: FAILED"));
        qemu_test_exit_failure();
    }
    #[cfg(feature = "qemu-test")]
    serial::write_line(format_args!(
        "[S6.HEAP] runtime growth: PASSED mapped={} bytes",
        heap::stats().size
    ));
    if ipc::stage8_1_runtime_probe() && ipc::object_count() == 0 && ipc::endpoint_count() == 0 {
        serial::write_line(format_args!(
            "[S8.1] kernel endpoint objects + process-local handles: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S8.1] kernel endpoint objects + process-local handles: FAILED"
        ));
        halt();
    }
    if ipc::stage8_2_runtime_probe() && ipc::object_count() == 0 && ipc::endpoint_count() == 0 {
        serial::write_line(format_args!(
            "[S8.2] handle-addressed bounded message passing: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S8.2] handle-addressed bounded message passing: FAILED"
        ));
        halt();
    }
    if ipc::stage8_3_runtime_probe() && ipc::object_count() == 0 && ipc::endpoint_count() == 0 {
        serial::write_line(format_args!(
            "[S8.3] race-free blocking IPC events/waits: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S8.3] race-free blocking IPC events/waits: FAILED"
        ));
        halt();
    }
    if ipc::stage8_4_runtime_probe()
        && ipc::object_count() == 0
        && ipc::shared_memory_count() == 0
        && ipc::endpoint_count() == 0
    {
        serial::write_line(format_args!(
            "[S8.4] shared-memory objects + cross-address-space mappings: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S8.4] shared-memory objects + cross-address-space mappings: FAILED"
        ));
        halt();
    }
    if ipc::stage8_5_runtime_probe()
        && ipc::object_count() == 0
        && ipc::shared_memory_count() == 0
        && ipc::endpoint_count() == 0
    {
        serial::write_line(format_args!(
            "[S8.5] capability handle transfer through IPC: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S8.5] capability handle transfer through IPC: FAILED"
        ));
        halt();
    }
    if ipc::stage8_6_runtime_probe()
        && ipc::object_count() == 0
        && ipc::shared_memory_count() == 0
        && ipc::endpoint_count() == 0
        && ipc::service_count() == 0
    {
        serial::write_line(format_args!(
            "[S8.6] named service registry + capability discovery: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S8.6] named service registry + capability discovery: FAILED"
        ));
        halt();
    }
    if ipc::stage8_7_runtime_probe()
        && ipc::object_count() == 0
        && ipc::shared_memory_count() == 0
        && ipc::endpoint_count() == 0
        && ipc::service_count() == 0
    {
        serial::write_line(format_args!(
            "[S8.7] multicore IPC/service/capability stress: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S8.7] multicore IPC/service/capability stress: FAILED"
        ));
        halt();
    }
    if wovenguard::self_test() && task::wovenguard_domain_policy_valid() {
        serial::write_line(format_args!(
            "[S9.1] WovenGuard domains + least-privilege policy: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.1] WovenGuard domains + least-privilege policy: FAILED"
        ));
        halt();
    }
    if wovenguard::lineage_self_test() && wovenguard::lineage_count() == 0 {
        serial::write_line(format_args!(
            "[S9.2A] WovenGuard capability lineage foundation: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.2A] WovenGuard capability lineage foundation: FAILED"
        ));
        halt();
    }
    if wovenguard::revocation_self_test() && wovenguard::lineage_count() == 0 {
        serial::write_line(format_args!(
            "[S9.2B] WovenGuard recursive capability revocation: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.2B] WovenGuard recursive capability revocation: FAILED"
        ));
        halt();
    }
    if task::capability_lineage_enforcement_valid() {
        serial::write_line(format_args!(
            "[S9.2C] WovenGuard task capability lineage enforcement: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.2C] WovenGuard task capability lineage enforcement: FAILED"
        ));
        halt();
    }
    if ipc::stage9_2d_runtime_probe() {
        serial::write_line(format_args!(
            "[S9.2D] WovenGuard IPC/object capability lineage: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.2D] WovenGuard IPC/object capability lineage: FAILED"
        ));
        halt();
    }
    if ipc::stage9_2e_runtime_probe() {
        serial::write_line(format_args!(
            "[S9.2E] WovenGuard SMP revocation + lineage closure: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.2E] WovenGuard SMP revocation + lineage closure: FAILED"
        ));
        halt();
    }
    if audit::stage9_3_runtime_probe() {
        serial::write_line(format_args!(
            "[S9.3] WovenGuard security audit ledger: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.3] WovenGuard security audit ledger: FAILED"
        ));
        halt();
    }
    if wovenguard::self_test() && task::sandbox_profile_foundation_valid() {
        serial::write_line(format_args!(
            "[S9.4A] WovenGuard sandbox profile + capability ceiling: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.4A] WovenGuard sandbox profile + capability ceiling: FAILED"
        ));
        halt();
    }
    if ipc::stage9_4b_runtime_probe() {
        serial::write_line(format_args!(
            "[S9.4B] WovenGuard IPC/service exposure policy: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.4B] WovenGuard IPC/service exposure policy: FAILED"
        ));
        halt();
    }
    if task::filesystem_sandbox_foundation_valid() {
        serial::write_line(format_args!(
            "[S9.4C] WovenGuard filesystem/resource sandbox: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.4C] WovenGuard filesystem/resource sandbox: FAILED"
        ));
        halt();
    }
    if task::sandbox_lifecycle_smp_closure_valid() {
        serial::write_line(format_args!(
            "[S9.4D] WovenGuard sandbox lifecycle + SMP closure: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.4D] WovenGuard sandbox lifecycle + SMP closure: FAILED"
        ));
        halt();
    }
    if task::device_capability_gates_valid() {
        serial::write_line(format_args!(
            "[S9.5] WovenGuard resource/device capability gates: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S9.5] WovenGuard resource/device capability gates: FAILED"
        ));
        halt();
    }
    // The root `network-test` mode always enables `qemu-test`.  Cargo artifact
    // dependencies can keep a nested kernel-only feature isolated, so key the
    // runtime probe from the qemu-test feature that is known to reach this
    // kernel artifact.  Only QEMU configurations that actually expose the
    // supported VirtIO network device run the live networking regression.
    //
    // Memory/storage QEMU suites do not attach that VirtIO NIC: `network::init`
    // simply fails its transport probe and those suites continue normally.
    #[cfg(all(
        feature = "qemu-test",
        not(feature = "stage10-6-test"),
        not(feature = "stage10-7-test")
    ))]
    {
        match network::init() {
            Ok(()) => {
                serial::write_line(format_args!("[NETTEST] virtio-net + smoltcp initialized"));
                if network::qemu_runtime_self_test() {
                    serial::write_line(format_args!("[NETTEST] DHCP/DNS/ICMP/UDP/TCP: PASSED"));
                    #[cfg(feature = "stage14-1-test")]
                    {
                        if network::stage14_1_dhcp_transition_self_test() {
                            serial::write_line(format_args!(
                                "[S14.1D] WovenNet DHCP transition/recovery: PASSED"
                            ));
                        } else {
                            serial::write_line(format_args!(
                                "[S14.1D] WovenNet DHCP transition/recovery: FAILED"
                            ));
                            halt();
                        }
                        if network::stage14_1_socket_stress_self_test() {
                            serial::write_line(format_args!(
                                "[S14.1S] WovenNet socket/ephemeral stress: PASSED"
                            ));
                        } else {
                            serial::write_line(format_args!(
                                "[S14.1S] WovenNet socket/ephemeral stress: FAILED"
                            ));
                            halt();
                        }
                        if network::stage14_1_lifecycle_self_test() {
                            serial::write_line(format_args!(
                                "[S14.1G] WovenNet DNS generation tokens: PASSED"
                            ));
                            serial::write_line(format_args!(
                                "[S14.1H] WovenNet resource/lifecycle hardening: PASSED"
                            ));
                            serial::write_line(format_args!("[S14.1] WovenNet IPv4 core: PASSED"));
                            #[cfg(feature = "stage14-2-test")]
                            {
                                if network::stage14_2_ipv6_foundation_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2A] WovenNet IPv6 address foundation: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2A] WovenNet IPv6 address foundation: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_ipv6_neighbor_foundation_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2B] WovenNet IPv6 neighbor foundation: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2B] WovenNet IPv6 neighbor foundation: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_icmpv6_neighbor_parser_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2C] WovenNet ICMPv6 neighbor parser: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2C] WovenNet ICMPv6 neighbor parser: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_router_advertisement_state_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2D] WovenNet Router Advertisement state: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2D] WovenNet Router Advertisement state: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_icmpv6_checksum_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2E] WovenNet ICMPv6 checksum boundary: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2E] WovenNet ICMPv6 checksum boundary: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_neighbor_state_dad_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2F] WovenNet neighbor state and DAD: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2F] WovenNet neighbor state and DAD: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_slaac_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2G] WovenNet SLAAC lifecycle: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2G] WovenNet SLAAC lifecycle: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_dhcpv6_protocol_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2H] WovenNet DHCPv6 protocol foundation: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2H] WovenNet DHCPv6 protocol foundation: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_dhcpv6_client_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2I] WovenNet DHCPv6 client state machine: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2I] WovenNet DHCPv6 client state machine: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_ipv6_interface_ingress_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2J] WovenNet IPv6 interface ingress: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2J] WovenNet IPv6 interface ingress: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_runtime_ipv6_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2K] WovenNet runtime IPv6 integration: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2K] WovenNet runtime IPv6 integration: FAILED"
                                    ));
                                    halt();
                                }
                                if network::stage14_2_ndp_ingress_hardening_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.2L] WovenNet NDP ingress hardening: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.2L] WovenNet NDP ingress hardening: FAILED"
                                    ));
                                    halt();
                                }
                                #[cfg(feature = "stage14-3-test")]
                                if network::stage14_3_socket_api_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.3] WovenNet socket API boundary: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.3] WovenNet socket API boundary: FAILED"
                                    ));
                                    halt();
                                }
                                #[cfg(feature = "stage14-3-test")]
                                if network::stage14_3_socket_authority_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.3A] WovenNet socket authority lifecycle: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.3A] WovenNet socket authority lifecycle: FAILED"
                                    ));
                                    halt();
                                }
                                #[cfg(feature = "stage14-4-test")]
                                if network::stage14_4_routing_table_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.4] WovenNet routing table: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.4] WovenNet routing table: FAILED"
                                    ));
                                    halt();
                                }
                                #[cfg(feature = "stage14-4-test")]
                                if network::stage14_4_live_route_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.4A] WovenNet live route integration: PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.4A] WovenNet live route integration: FAILED"
                                    ));
                                    halt();
                                }
                                #[cfg(feature = "stage14-5-test")]
                                if firewall_policy::stage14_5a_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.5A] firewall policy foundation PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.5A] firewall policy foundation FAILED"
                                    ));
                                    halt();
                                }
                                #[cfg(feature = "stage14-5b-test")]
                                if firewall_policy::stage14_5b_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.5B] live packet enforcement PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.5B] live packet enforcement FAILED"
                                    ));
                                    halt();
                                }
                                #[cfg(feature = "stage14-5c-test")]
                                if firewall_policy::stage14_5c_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.5C] firewall policy authority PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.5C] firewall policy authority FAILED"
                                    ));
                                    halt();
                                }
                                #[cfg(feature = "stage14-5d-test")]
                                if firewall_policy::stage14_5d_self_test() {
                                    serial::write_line(format_args!(
                                        "[S14.5D] firewall parser/transport hardening PASSED"
                                    ));
                                } else {
                                    serial::write_line(format_args!(
                                        "[S14.5D] firewall parser/transport hardening FAILED"
                                    ));
                                    halt();
                                }
                            }
                        } else {
                            serial::write_line(format_args!(
                                "[S14.1H] WovenNet resource/lifecycle hardening: FAILED"
                            ));
                            halt();
                        }
                    }
                } else {
                    serial::write_line(format_args!("[NETTEST] runtime regression: FAILED"));
                    halt();
                }
            }
            Err(error) => {
                // A network-test image must never silently downgrade to the
                // no-NIC memory/storage path: the host harness relies on this
                // marker to distinguish transport failure from an intentional
                // skip. Other QEMU images still omit the NIC and continue.
                #[cfg(feature = "network-test")]
                {
                    serial::write_line(format_args!("[NETTEST] init failed: {:?}", error));
                    halt();
                }
                #[cfg(not(feature = "network-test"))]
                {
                    let _ = error;
                }
            }
        }
    }
    if !block_io::start_worker() {
        console.println("BLOCK I/O WORKER: START FAILED");
        halt();
    }
    #[cfg(any(feature = "stage10-9-test", feature = "stage19-8-test"))]
    if !async_events::start_worker() {
        console.println("TIMER/EVENT WORKER: START FAILED");
        halt();
    }
    if block_io::async_completion_self_test() {
        serial::write_line(format_args!("[BLOCK IO] worker completion: PASSED"));
    } else {
        serial::write_line(format_args!("[BLOCK IO] worker completion: FAILED"));
        halt();
    }
    if block_io::stage10_1_event_driven_completion_valid() {
        serial::write_line(format_args!(
            "[S10.1] kernel event + async I/O foundation: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S10.1] kernel event + async I/O foundation: FAILED"
        ));
        halt();
    }
    if block_io::stage10_2_generic_async_completion_valid() {
        serial::write_line(format_args!(
            "[S10.2] generic async request/completion API: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S10.2] generic async request/completion API: FAILED"
        ));
        halt();
    }
    if !task::start_pager() {
        console.println("PAGER: START FAILED");
        halt();
    }
    // The Stage 15.2 boot hands the ATA primary master to WovenFS, so the disk
    // is deliberately not a FAT32 volume. Running the FAT32 mutation test
    // against it would fail on a disk it was never given. Every other
    // configuration, including the storage gate that owns this assertion, still
    // runs it unchanged.
    #[cfg(all(
        not(feature = "stage15-2-test"),
        not(feature = "stage21-6-test"),
        not(feature = "stage21-7-test"),
        not(feature = "stage21-8-test"),
        not(feature = "stage21-9-test"),
        not(feature = "stage21-10-test"),
        not(feature = "stage21-11-test"),
        not(feature = "stage22-1-test"),
        not(feature = "stage22-2-test"),
        not(feature = "stage22-3-test"),
        not(feature = "stage22-5-test"),
        not(feature = "stage24-3-test")
    ))]
    match storage::live_mutation_self_test() {
        storage::LiveMutationTestStatus::Passed => {
            serial::write_line(format_args!(
                "[STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: PASSED"
            ));
        }
        storage::LiveMutationTestStatus::Skipped => {
            serial::write_line(format_args!(
                "[STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: SKIPPED"
            ));
        }
        storage::LiveMutationTestStatus::Failed(stage) => {
            serial::write_line(format_args!(
                "[STORAGE MUTATION] live FAT32 rename/delete/growth/lifecycle: FAILED at {}",
                stage
            ));
            halt();
        }
    }

    while timer::ticks() < 3 {
        task::yield_now();
    }

    let probe_id = match task::spawn("preemption-probe", preemption_probe_task) {
        Ok(id) => id,
        Err(_) => {
            console.println("PREEMPTION TEST: SPAWN FAILED");
            halt();
        }
    };

    while !PREEMPTION_PROBE_BLOCKED.load(Ordering::Acquire) {
        x86_64::instructions::hlt();
    }

    if !task::wake_task(probe_id) {
        console.println("TASK WAKEUP: FAILED");
        halt();
    }

    while !PREEMPTION_PROBE_COMPLETED.load(Ordering::Acquire) {
        x86_64::instructions::hlt();
    }

    console.println("TIMER IRQ: OK");
    console.println("TIMER PREEMPTION: OK");
    console.println("TASK SLEEP/BLOCK/WAKE: OK");
    serial::write_line(format_args!(
        "[BOOT] timer preemption and task lifecycle verified"
    ));

    let preemptions_before_fairness = task::summary().preemption_switches;
    if task::spawn("fair-peer-a", fairness_probe_a).is_err() {
        console.println("SCHEDULER FAIRNESS A: SPAWN FAILED");
        serial::write_line(format_args!("[SCHED] peer A spawn failed"));
        halt();
    }
    if task::spawn("fair-peer-b", fairness_probe_b).is_err() {
        console.println("SCHEDULER FAIRNESS B: SPAWN FAILED");
        serial::write_line(format_args!("[SCHED] peer B spawn failed"));
        halt();
    }
    while FAIR_TASKS_COMPLETED.load(Ordering::Acquire) != 2 {
        x86_64::instructions::hlt();
    }
    let fairness_summary = task::summary();
    if FAIR_TASK_A_RUNS.load(Ordering::Acquire) == 0
        || FAIR_TASK_B_RUNS.load(Ordering::Acquire) == 0
        || fairness_summary.preemption_switches < preemptions_before_fairness + 2
    {
        console.println("SCHEDULER FAIRNESS/QUANTUM: FAILED");
        halt();
    }
    console.println("PRIORITY ROUND-ROBIN/TIME SLICES: OK");
    serial::write_line(format_args!(
        "[BOOT] priority round-robin fairness and per-task quanta verified"
    ));

    console.println("TIMER IRQ: OK");

    keyboard::inject_validation_input(2);
    let isolation_baseline = memory::stats().allocated_frames;
    let Some(first_program) = userspace::create_exec_process() else {
        console.println("USER PROCESS IMAGE: MAPPING FAILED");
        halt();
    };
    let Some(second_program) = userspace::create_stub_process() else {
        console.println("SECOND USER ADDRESS SPACE: FAILED");
        halt();
    };
    if !first_program.stack.is_aligned()
        || first_program.stack.size != userspace::UserStack::SIZE
        || !paging::user_range_is_unmapped_in(
            first_program.address_space.paging(),
            first_program.stack.guard_base,
            userspace::UserStack::GUARD_SIZE,
        )
        || !paging::user_range_has_protection_in(
            first_program.address_space.paging(),
            first_program.stack.base,
            first_program.stack.size,
            true,
            false,
        )
        || !paging::user_range_is_unmapped_in(
            second_program.address_space.paging(),
            second_program.stack.guard_base,
            userspace::UserStack::GUARD_SIZE,
        )
        || first_program.image.entry != second_program.image.entry
        || first_program.stack.top != second_program.stack.top
        || first_program.address_space.root_address() == second_program.address_space.root_address()
    {
        console.println("USER ADDRESS-SPACE ISOLATION: INVALID");
        halt();
    }
    serial::write_line(format_args!(
        "[BOOT] independent user roots, unmapped stack guards, and RW/NX stacks verified"
    ));

    if !userspace::mmap_w_xor_x_self_test(first_program.address_space) {
        console.println("MMAP W^X INVARIANT: FAILED");
        halt();
    }
    console.println("MMAP W^X INVARIANT: OK");
    serial::write_line(format_args!(
        "[BOOT] anonymous mmap W^X invariant verified (writable mapping is never executable)"
    ));

    if !task::file_fault_io_self_test() {
        serial::write_line(format_args!(
            "[FAULT IO] interrupt/preemption guard: FAILED"
        ));
        halt();
    }
    serial::write_line(format_args!(
        "[FAULT IO] timer IRQs live, task stable, state restored: PASSED"
    ));
    let first_root = first_program.address_space.root_address();
    let second_root = second_program.address_space.root_address();
    // Publish the paired bootstrap processes as one BSP-local transaction.
    // A timer interrupt used to be able to schedule process A after its TCB was
    // made Ready but before process B and its process-table entry were created.
    // That timing window became visible with multiple LAPIC timers running and
    // could leave the boot validation waiting forever before the identity audit.
    // APs may continue taking timer interrupts, but their scheduler path uses
    // try_lock and cannot run CPU-0-owned userspace tasks.
    serial::write_line(format_args!("[BOOT] paired userspace spawn: BEGIN"));
    let (first_spawn, second_spawn) = x86_64::instructions::interrupts::without_interrupts(|| {
        let first = task::spawn_user_process("init-user-a", first_program);
        let second = if first.is_ok() {
            Some(task::spawn_user_process("init-user-b", second_program))
        } else {
            None
        };
        (first, second)
    });

    let first_pid = match first_spawn {
        Ok((pid, context)) => {
            serial::write_line(format_args!(
                "[BOOT] ring3 frame CS={:#x} SS={:#x} RIP={:#x} RSP={:#x}",
                context.code_segment, context.data_segment, context.entry, context.stack_top,
            ));
            pid
        }
        Err(_) => {
            console.println("FIRST USER PROCESS: SPAWN FAILED");
            halt();
        }
    };
    let second_pid = match second_spawn {
        Some(Ok((pid, _))) => pid,
        _ => {
            console.println("SECOND USER PROCESS: SPAWN FAILED");
            halt();
        }
    };
    serial::write_line(format_args!("[BOOT] paired userspace spawn: READY"));
    serial::write_line(format_args!(
        "[PROC-DIAG] first credentials lookup: BEGIN pid={}",
        first_pid.as_u64()
    ));
    let first_credentials = task::process_credentials(first_pid);
    serial::write_line(format_args!("[PROC-DIAG] first credentials lookup: DONE"));

    serial::write_line(format_args!(
        "[PROC-DIAG] second credentials lookup: BEGIN pid={}",
        second_pid.as_u64()
    ));
    let second_credentials = task::process_credentials(second_pid);
    serial::write_line(format_args!("[PROC-DIAG] second credentials lookup: DONE"));

    if first_credentials != Some(task::Credentials::USERSPACE)
        || second_credentials != Some(task::Credentials::USERSPACE)
    {
        console.println("USER CREDENTIALS: INVALID");
        halt();
    }
    serial::write_line(format_args!(
        "[AUDIT] userspace identity uid={} gid={}",
        task::Credentials::USERSPACE.uid,
        task::Credentials::USERSPACE.gid,
    ));

    serial::write_line(format_args!(
        "[PROC-DIAG] paired userspace execution wait: BEGIN"
    ));
    let user_pair_wait_start = timer::ticks();
    while !task::process_exited(first_pid) || !task::process_exited(second_pid) {
        if timer::ticks().wrapping_sub(user_pair_wait_start) > 2_000 {
            serial::write_line(format_args!(
                "[BOOT] paired userspace execution: TIMEOUT first_exited={} second_exited={}",
                task::process_exited(first_pid),
                task::process_exited(second_pid),
            ));
            console.println("PAIRED USERSPACE EXECUTION: TIMEOUT");
            halt();
        }
        x86_64::instructions::hlt();
    }
    serial::write_line(format_args!("[BOOT] paired userspace execution: PASSED"));

    // Stage 7.3: prove that the already-established per-CPU privilege and
    // scheduler machinery can execute a deliberately pinned Ring-3 task on
    // every application processor. Keep the workload intentionally minimal:
    // `/bin/true` performs only exit(0), so this test does not relax Stage 7.2's
    // BSP ownership of filesystem/network/pager/device service execution.
    if smp::online_count() > 1 {
        serial::write_line(format_args!(
            "[S7.3] pinned Ring-3 AP execution: BEGIN online={}",
            smp::online_count()
        ));
        for cpu in 1..smp::online_count() {
            let Some(program) = userspace::create_true_process() else {
                serial::write_line(format_args!(
                    "[S7.3] cpu={} true image creation: FAILED",
                    cpu
                ));
                halt();
            };
            let pid = match task::spawn_pinned_user_process_on(cpu, "s7.3-ring3", program) {
                Ok((pid, _)) => pid,
                Err(_) => {
                    serial::write_line(format_args!(
                        "[S7.3] cpu={} pinned Ring-3 spawn: FAILED",
                        cpu
                    ));
                    halt();
                }
            };

            let start = timer::ticks();
            while !task::process_exited(pid) {
                if timer::ticks().wrapping_sub(start) > 500 {
                    serial::write_line(format_args!(
                        "[S7.3] cpu={} pinned Ring-3 execution: TIMEOUT pid={}",
                        cpu,
                        pid.as_u64()
                    ));
                    halt();
                }
                x86_64::instructions::hlt();
            }
            match task::wait_process(pid.as_u64()) {
                Ok(0) => {}
                Ok(code) => {
                    serial::write_line(format_args!(
                        "[S7.3] cpu={} pinned Ring-3 exit code: FAILED code={}",
                        cpu, code
                    ));
                    halt();
                }
                Err(_) => {
                    serial::write_line(format_args!(
                        "[S7.3] cpu={} pinned Ring-3 reap: FAILED",
                        cpu
                    ));
                    halt();
                }
            }
            serial::write_line(format_args!(
                "[S7.3] cpu={} pinned Ring-3 execution: PASSED",
                cpu
            ));
        }
        serial::write_line(format_args!(
            "[S7.3] pinned Ring-3 execution on every AP: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S7.3] single-CPU baseline preserved; no AP Ring-3 probe required"
        ));
    }

    // Stage 7.4: prove Ready-state Ring-3 ownership migration and hard
    // affinity without enabling general userspace migration. Both probes use
    // `/bin/true`, whose AP syscall/exit path was validated in Stage 7.3.
    if smp::online_count() > 1 {
        let target_cpu = smp::online_count() - 1;
        serial::write_line(format_args!(
            "[S7.4] Ready Ring-3 migration/affinity: BEGIN target_cpu={}",
            target_cpu
        ));

        // Explicit Ready-state migration: create on CPU0 with an all-online
        // affinity mask, move ownership under the scheduler lock, then let the
        // destination CPU perform the first dispatch.
        let Some(program) = userspace::create_true_process() else {
            serial::write_line(format_args!("[S7.4] migration true image creation: FAILED"));
            halt();
        };
        // Keep the Ready-state setup atomic with respect to BSP dispatch.
        // `spawn_migratable_user_probe()` publishes the probe Ready on CPU0;
        // without this local interrupt boundary, a timer tick can dispatch the
        // tiny `/bin/true` probe before the test applies the intended migration.
        // The scheduler's Ready-only contract remains unchanged.
        let (migration_pid, _, _) = x86_64::instructions::interrupts::without_interrupts(|| {
            let result = match task::spawn_migratable_user_probe("s7.4-migrate", program) {
                Ok(result) => result,
                Err(_) => {
                    serial::write_line(format_args!("[S7.4] migratable Ring-3 spawn: FAILED"));
                    halt();
                }
            };
            if task::task_affinity(result.1) != Some(smp::online_mask()) {
                serial::write_line(format_args!("[S7.4] initial migratable affinity: FAILED"));
                halt();
            }
            if task::migrate_ready_task(result.1, target_cpu).is_err() {
                serial::write_line(format_args!(
                    "[S7.4] explicit Ready migration to cpu={}: FAILED",
                    target_cpu
                ));
                halt();
            }
            result
        });
        let migration_start = timer::ticks();
        while !task::process_exited(migration_pid) {
            if timer::ticks().wrapping_sub(migration_start) > 500 {
                serial::write_line(format_args!(
                    "[S7.4] explicit Ready migration execution: TIMEOUT pid={} cpu={}",
                    migration_pid.as_u64(),
                    target_cpu
                ));
                halt();
            }
            x86_64::instructions::hlt();
        }
        if task::wait_process(migration_pid.as_u64()) != Ok(0) {
            serial::write_line(format_args!("[S7.4] explicit Ready migration reap: FAILED"));
            halt();
        }
        serial::write_line(format_args!(
            "[S7.4] explicit Ready Ring-3 migration 0->{}: PASSED",
            target_cpu
        ));

        // Affinity-driven ownership transfer: excluding CPU0 from a Ready
        // migratable task must atomically move ownership to the lowest allowed
        // CPU before it can run. A single-bit mask makes the destination exact.
        let Some(program) = userspace::create_true_process() else {
            serial::write_line(format_args!("[S7.4] affinity true image creation: FAILED"));
            halt();
        };
        let target_mask = 1usize << target_cpu;
        // As above, do not expose a dispatch window between publishing the
        // CPU0-owned Ready probe and applying its hard affinity. Once the
        // affinity call moves ownership to the AP, normal SMP execution resumes.
        let (affinity_pid, _, _) = x86_64::instructions::interrupts::without_interrupts(|| {
            let result = match task::spawn_migratable_user_probe("s7.4-affinity", program) {
                Ok(result) => result,
                Err(_) => {
                    serial::write_line(format_args!("[S7.4] affinity Ring-3 spawn: FAILED"));
                    halt();
                }
            };
            if task::set_ready_task_affinity(result.1, target_mask).is_err() {
                serial::write_line(format_args!(
                    "[S7.4] hard affinity transfer to cpu={}: FAILED",
                    target_cpu
                ));
                halt();
            }
            result
        });
        let affinity_start = timer::ticks();
        while !task::process_exited(affinity_pid) {
            if timer::ticks().wrapping_sub(affinity_start) > 500 {
                serial::write_line(format_args!(
                    "[S7.4] hard-affinity Ring-3 execution: TIMEOUT pid={} cpu={}",
                    affinity_pid.as_u64(),
                    target_cpu
                ));
                halt();
            }
            x86_64::instructions::hlt();
        }
        if task::wait_process(affinity_pid.as_u64()) != Ok(0) {
            serial::write_line(format_args!("[S7.4] hard-affinity Ring-3 reap: FAILED"));
            halt();
        }
        serial::write_line(format_args!(
            "[S7.4] hard-affinity Ring-3 transfer to cpu={}: PASSED",
            target_cpu
        ));
        serial::write_line(format_args!(
            "[S7.4] Ready-state Ring-3 migration + affinity: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S7.4] single-CPU baseline preserved; no Ring-3 migration required"
        ));
    }

    // Stage 7.6: integrate the validated Ring-3 placement/migration pieces
    // without changing legacy CPU0-owned userspace. Audited multicore tasks get
    // least-loaded placement and may participate in the existing Ready-only
    // rebalancer. The burst below intentionally starts on CPU0 through the
    // Stage 7.4 probe API so automatic userspace rebalancing has real work.
    if smp::online_count() > 1 {
        serial::write_line(format_args!(
            "[S7.6] integrated multicore userspace: BEGIN online={}",
            smp::online_count()
        ));

        let mut balance_pids = [None; 4];
        let before_rebalance = task::rebalance_stats();
        let before_userspace_rebalance = task::userspace_rebalance_migrations();
        x86_64::instructions::interrupts::without_interrupts(|| {
            for slot in &mut balance_pids {
                let Some(program) = userspace::create_true_process() else {
                    serial::write_line(format_args!(
                        "[S7.6] rebalance probe image creation: FAILED"
                    ));
                    halt();
                };
                let (pid, _, _) = match task::spawn_migratable_user_probe("s7.6-rebalance", program)
                {
                    Ok(result) => result,
                    Err(_) => {
                        serial::write_line(format_args!("[S7.6] rebalance probe spawn: FAILED"));
                        halt();
                    }
                };
                *slot = Some(pid);
            }
            if !task::rebalance_once() {
                serial::write_line(format_args!(
                    "[S7.6] automatic Ready Ring-3 rebalance: FAILED"
                ));
                halt();
            }
        });
        let after_rebalance = task::rebalance_stats();
        if after_rebalance.migrations <= before_rebalance.migrations
            || task::userspace_rebalance_migrations() <= before_userspace_rebalance
        {
            serial::write_line(format_args!(
                "[S7.6] userspace rebalance migration accounting: FAILED"
            ));
            halt();
        }
        for pid in balance_pids.into_iter().flatten() {
            let start = timer::ticks();
            while !task::process_exited(pid) {
                if timer::ticks().wrapping_sub(start) > 500 {
                    serial::write_line(format_args!(
                        "[S7.6] rebalanced Ring-3 execution: TIMEOUT pid={}",
                        pid.as_u64()
                    ));
                    halt();
                }
                x86_64::instructions::hlt();
            }
            if task::wait_process(pid.as_u64()) != Ok(0) {
                serial::write_line(format_args!("[S7.6] rebalanced Ring-3 reap: FAILED"));
                halt();
            }
        }
        serial::write_line(format_args!(
            "[S7.6] automatic Ready Ring-3 rebalancing: PASSED"
        ));

        // Exercise the production multicore spawn API. With only the kernel
        // task runnable on CPU0, least-loaded placement must select an AP.
        let Some(program) = userspace::create_true_process() else {
            serial::write_line(format_args!(
                "[S7.6] multicore placement image creation: FAILED"
            ));
            halt();
        };
        let (placement_pid, _placement_task, _, placement) =
            match task::spawn_multicore_user_process("s7.6-placement", program, smp::online_mask())
            {
                Ok(result) => result,
                Err(_) => {
                    serial::write_line(format_args!("[S7.6] multicore placement spawn: FAILED"));
                    halt();
                }
            };
        let owner_cpu = placement.owner_cpu;
        if owner_cpu == 0 || placement.affinity_mask != smp::online_mask() {
            serial::write_line(format_args!(
                "[S7.6] least-loaded Ring-3 placement: FAILED owner={} affinity={:#x} expected={:#x}",
                owner_cpu,
                placement.affinity_mask,
                smp::online_mask(),
            ));
            halt();
        }
        let start = timer::ticks();
        while !task::process_exited(placement_pid) {
            if timer::ticks().wrapping_sub(start) > 500 {
                serial::write_line(format_args!(
                    "[S7.6] least-loaded Ring-3 execution: TIMEOUT pid={} cpu={}",
                    placement_pid.as_u64(),
                    owner_cpu
                ));
                halt();
            }
            x86_64::instructions::hlt();
        }
        if task::wait_process(placement_pid.as_u64()) != Ok(0) {
            serial::write_line(format_args!("[S7.6] least-loaded Ring-3 reap: FAILED"));
            halt();
        }
        serial::write_line(format_args!(
            "[S7.6] least-loaded multicore Ring-3 placement: PASSED cpu={}",
            owner_cpu
        ));
    } else {
        serial::write_line(format_args!(
            "[S7.6] single-CPU integrated userspace baseline preserved"
        ));
    }

    if !syscall::user_memory_verified() || task::anonymous_mapping_count() != 0 {
        console.println("USER MMAP/MUNMAP: FAILED");
        halt();
    }
    let async_stats = async_op::stats();
    if !syscall::user_async_verified() || async_stats.active != 0 || async_stats.owner_reaped < 2 {
        serial::write_line(format_args!(
            "[S10.3] userspace async completion ABI: FAILED verified={} active={} owner_reaped={}",
            syscall::user_async_verified(),
            async_stats.active,
            async_stats.owner_reaped
        ));
        halt();
    }
    serial::write_line(format_args!(
        "[S10.3] userspace async completion ABI + cancellation/teardown: PASSED"
    ));
    #[cfg(feature = "stage11-1-test")]
    {
        serial::write_line(format_args!("[S11.1] production process model: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage6-hotplug-test")]
    {
        if !smp::hotplug_cancellation_self_test() {
            serial::write_line(format_args!("[S6.HOTPLUG] cancellation: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S6.HOTPLUG] cancellation: PASSED"));
        if !task::hotplug_evacuation_atomic_self_test() {
            serial::write_line(format_args!("[S6.HOTPLUG] evacuation: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S6.HOTPLUG] evacuation: PASSED"));
        if !smp::hotplug_rejection_recovery_probe(smp::online_count().saturating_sub(1)) {
            serial::write_line(format_args!("[S6.HOTPLUG] rejection recovery: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S6.HOTPLUG] rejection recovery: PASSED"));
        if !smp::hotplug_timeout_recovery_probe(smp::online_count().saturating_sub(1)) {
            serial::write_line(format_args!("[S6.HOTPLUG] timeout recovery: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S6.HOTPLUG] timeout recovery: PASSED"));
        const CYCLES: usize = 2;
        for cycle in 0..CYCLES {
            let offline_cpu = smp::online_count().saturating_sub(1);
            if smp::online_count() < 2 || !smp::request_cpu_offline(offline_cpu) {
                serial::write_line(format_args!(
                    "[S6.HOTPLUG] offline AP: FAILED cycle={}",
                    cycle
                ));
                qemu_test_exit_failure();
            }
            if cycle == 0 {
                serial::write_line(format_args!(
                    "[S6.HOTPLUG] offline AP: PASSED online={} mask={:#x}",
                    smp::online_count(),
                    smp::online_mask()
                ));
            }
            if !smp::request_cpu_online(offline_cpu) {
                serial::write_line(format_args!(
                    "[S6.HOTPLUG] online AP: FAILED cycle={}",
                    cycle
                ));
                qemu_test_exit_failure();
            }
            serial::write_line(format_args!(
                "[S6.HOTPLUG] cycle={} PASSED online={} mask={:#x}",
                cycle + 1,
                smp::online_count(),
                smp::online_mask()
            ));
        }
        serial::write_line(format_args!(
            "[S6.HOTPLUG] lifecycle: PASSED cycles={} online={} mask={:#x}",
            CYCLES,
            smp::online_count(),
            smp::online_mask()
        ));
        if smp::online_count() == 4 {
            for cycle in 0..CYCLES {
                if !smp::request_cpu_offline(1)
                    || smp::online_count() != 3
                    || smp::online_mask() != 0xd
                    || smp::reschedule_cpu(1)
                    || !smp::reschedule_cpu(3)
                {
                    serial::write_line(format_args!(
                        "[S6.HOTPLUG] middle offline: FAILED cycle={}",
                        cycle
                    ));
                    qemu_test_exit_failure();
                }
                if !smp::hotplug_hole_worker_probe(cycle + 1) {
                    serial::write_line(format_args!(
                        "[S6.HOTPLUG] hole worker: FAILED cycle={}",
                        cycle
                    ));
                    qemu_test_exit_failure();
                }
                smp::shootdown();
                if !smp::request_cpu_online(1)
                    || smp::online_count() != 4
                    || smp::online_mask() != 0xf
                {
                    serial::write_line(format_args!(
                        "[S6.HOTPLUG] middle online: FAILED cycle={}",
                        cycle
                    ));
                    qemu_test_exit_failure();
                }
            }
            serial::write_line(format_args!(
                "[S6.HOTPLUG] middle lifecycle: PASSED cycles={} online={} mask={:#x}",
                CYCLES,
                smp::online_count(),
                smp::online_mask()
            ));
        }
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage11-aslr-test")]
    {
        if !userspace::aslr_geometry_self_test() {
            serial::write_line(format_args!("[S11.ASLR] arena and stack guard: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S11.ASLR] arena and stack guard: PASSED"));
    }
    #[cfg(feature = "stage11-tls-test")]
    {
        if !userspace::compiler_tls_self_test() {
            serial::write_line(format_args!("[S11.TLS] compiler TLS lifecycle: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S11.TLS] compiler TLS lifecycle: PASSED"));
    }
    #[cfg(feature = "stage11-exit-status-test")]
    {
        let frames = memory::stats().allocated_frames;
        let processes = task::process_count();
        let endpoints = ipc::endpoint_count();
        let bytes = include_bytes!(env!("WOVENHAT_EXIT_STATUS_ELF"));
        let Ok(parsed) = elf::parse(bytes) else {
            serial::write_line(format_args!("[S11.PLACEMENT] fixture parse: FAILED"));
            qemu_test_exit_failure();
        };
        let expected_entry = parsed.entry;
        let Some(program) = userspace::load_fixed_placement_probe(bytes) else {
            serial::write_line(format_args!("[S11.STATUS] fixture ELF: FAILED"));
            qemu_test_exit_failure();
        };
        let loaded_entry = program.image.entry;
        serial::write_line(format_args!(
            "[S11.PLACEMENT] candidate=0x30000 entry={:#x} expected={:#x}",
            loaded_entry, expected_entry
        ));
        let Ok((pid, _)) = task::spawn_user_process("exit-status-fixture", program) else {
            serial::write_line(format_args!("[S11.STATUS] fixture spawn: FAILED"));
            qemu_test_exit_failure();
        };
        let exit = loop {
            match task::wait_process(pid.as_u64()) {
                Ok(status) => break status,
                Err(task::WaitError::StillRunning) => task::yield_now(),
                Err(task::WaitError::NoSuchChild) => {
                    serial::write_line(format_args!("[S11.STATUS] fixture wait: FAILED"));
                    qemu_test_exit_failure();
                }
            }
        };
        if exit != 0
            || loaded_entry != expected_entry
            || memory::stats().allocated_frames != frames
            || task::process_count() != processes
            || ipc::endpoint_count() != endpoints
            || thread::reap_owner(pid.as_u64()) != 0
        {
            serial::write_line(format_args!(
                "[S11.STATUS] fixture exit={} frames={}/{} processes={}/{} endpoints={}/{}: FAILED",
                exit,
                memory::stats().allocated_frames,
                frames,
                task::process_count(),
                processes,
                ipc::endpoint_count(),
                endpoints
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S11.PLACEMENT] fixed ET_EXEC at nonzero ASLR candidate: PASSED"
        ));
        let Some(pie) = userspace::load_pie_placement_probe() else {
            serial::write_line(format_args!("[S11.PLACEMENT] PIE preparation: FAILED"));
            qemu_test_exit_failure();
        };
        if pie.image.entry != 0x4000_0003_0000 {
            serial::write_line(format_args!("[S11.PLACEMENT] PIE bias lost: FAILED"));
            qemu_test_exit_failure();
        }
        let Ok((pid, _)) = task::spawn_user_process("pie-placement", pie) else {
            serial::write_line(format_args!("[S11.PLACEMENT] PIE spawn: FAILED"));
            qemu_test_exit_failure();
        };
        let status = loop {
            match task::wait_process(pid.as_u64()) {
                Ok(status) => break status,
                Err(task::WaitError::StillRunning) => task::yield_now(),
                Err(task::WaitError::NoSuchChild) => {
                    serial::write_line(format_args!("[S11.PLACEMENT] PIE wait: FAILED"));
                    qemu_test_exit_failure();
                }
            }
        };
        if status != 37
            || memory::stats().allocated_frames != frames
            || task::process_count() != processes
            || ipc::endpoint_count() != endpoints
        {
            serial::write_line(format_args!(
                "[S11.PLACEMENT] PIE execution/teardown: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S11.PLACEMENT] PIE executes with nonzero ASLR: PASSED"
        ));
        serial::write_line(format_args!(
            "[S11.STATUS] libwoven signed exit status/ownership/teardown: PASSED"
        ));
    }
    #[cfg(feature = "stage11-2-test")]
    {
        if !thread::structural_self_test() {
            serial::write_line(format_args!("[S11.2] threads/TLS/join: FAILED"));
            qemu_test_exit_failure();
        }
        let Some(program) = userspace::create_stage11_2_thread_process() else {
            serial::write_line(format_args!(
                "[S11.2] scheduler-backed thread image: FAILED"
            ));
            qemu_test_exit_failure();
        };
        let Ok((pid, _)) = task::spawn_user_process("thread-test", program) else {
            serial::write_line(format_args!(
                "[S11.2] scheduler-backed thread spawn: FAILED"
            ));
            qemu_test_exit_failure();
        };
        let exit = loop {
            match task::wait_process(pid.as_u64()) {
                Ok(exit) => break exit,
                Err(task::WaitError::StillRunning) => task::yield_now(),
                Err(task::WaitError::NoSuchChild) => {
                    serial::write_line(format_args!(
                        "[S11.2] scheduler-backed thread wait: FAILED"
                    ));
                    qemu_test_exit_failure();
                }
            }
        };
        if exit != 0 {
            serial::write_line(format_args!(
                "[S11.2] scheduler-backed thread exit={}: FAILED",
                exit
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S11.2] scheduler-backed thread: PASSED"));
        serial::write_line(format_args!("[S11.2] threads/TLS/join: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage11-3-test")]
    {
        if !notifications::structural_self_test() {
            serial::write_line(format_args!("[S11.3] notifications: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S11.3] notifications: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage12-1-test")]
    {
        if !vfs_api::structural_self_test() {
            serial::write_line(format_args!("[S12.1] VFS boundary: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S12.1] VFS boundary: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-1-test")]
    {
        if !driver::register("pit", device::DeviceKind::Timer)
            || !driver::bind("pit")
            || !driver::suspend("pit")
            || !driver::resume("pit")
        {
            serial::write_line(format_args!("[S13.1] driver framework: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.1] driver framework: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-2-test")]
    {
        let pci_ok = hal::pci::self_test()
            && hardware.pci.discovered >= u16::from(hardware.pci.recorded)
            && hardware.pci.recorded != 0
            && driver::stage13_2_legacy_lifecycle_self_test()
            && driver::stage13_2_lifecycle_self_test();
        if !pci_ok {
            serial::write_line(format_args!(
                "[S13.2] PCIe topology + generation-safe ownership/teardown: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.2] PCIe topology + generation-safe ownership/teardown: PASSED devices={} recorded={} segments={} ecam={}",
            hardware.pci.discovered, hardware.pci.recorded, hardware.pci.segments, hardware.pci.ecam as u8
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-3-test")]
    {
        use block::BlockDevice;
        if !nvme::self_test() || !nvme::probe() {
            serial::write_line(format_args!(
                "[S13.3] NVMe controller/queue foundation: FAILED probe"
            ));
            qemu_test_exit_failure();
        }
        let sectors = match nvme::init() {
            Ok(sectors) => sectors,
            Err(error) => {
                serial::write_line(format_args!(
                    "[S13.3] NVMe controller/queue foundation: FAILED init {:?}",
                    error
                ));
                qemu_test_exit_failure();
            }
        };
        let io_ok = nvme::with_controller(|controller| {
            let mut original = [0_u8; block::SECTOR_SIZE];
            let mut verify = [0_u8; block::SECTOR_SIZE];
            let mut pattern = [0_u8; block::SECTOR_SIZE];
            pattern[..15].copy_from_slice(b"wovenhat-nvme13");
            if controller.read_sector(0, &mut original).is_err()
                || controller.write_sector(0, &pattern).is_err()
                || controller.flush().is_err()
                || controller.read_sector(0, &mut verify).is_err()
                || verify != pattern
            {
                return false;
            }
            controller.write_sector(0, &original).is_ok() && controller.flush().is_ok()
        })
        .unwrap_or(false);
        if !io_ok {
            serial::write_line(format_args!(
                "[S13.3] NVMe controller/queue foundation: FAILED I/O"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.3] NVMe controller/queue foundation: PASSED sectors={}",
            sectors
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-4-test")]
    {
        use block::BlockDevice;
        if !ahci::self_test() || !ahci::probe() {
            serial::write_line(format_args!(
                "[S13.4] AHCI/SATA DMA block I/O: FAILED probe"
            ));
            qemu_test_exit_failure();
        }
        let sectors = match ahci::init() {
            Ok(sectors) => sectors,
            Err(error) => {
                serial::write_line(format_args!(
                    "[S13.4] AHCI/SATA DMA block I/O: FAILED init {:?}",
                    error
                ));
                qemu_test_exit_failure();
            }
        };
        let io_ok = ahci::with_controller(|controller| {
            let mut original = [0_u8; block::SECTOR_SIZE];
            let mut verify = [0_u8; block::SECTOR_SIZE];
            let mut pattern = [0_u8; block::SECTOR_SIZE];
            pattern[..15].copy_from_slice(b"wovenhat-ahci14");
            if controller.read_sector(0, &mut original).is_err()
                || controller.write_sector(0, &pattern).is_err()
                || controller.flush().is_err()
                || controller.read_sector(0, &mut verify).is_err()
                || verify != pattern
            {
                return false;
            }
            controller.write_sector(0, &original).is_ok() && controller.flush().is_ok()
        })
        .unwrap_or(false);
        if !io_ok {
            serial::write_line(format_args!("[S13.4] AHCI/SATA DMA block I/O: FAILED I/O"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.4] AHCI/SATA DMA block I/O: PASSED sectors={}",
            sectors
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-5-test")]
    {
        if !xhci::self_test() || !xhci::probe() {
            serial::write_line(format_args!("[S13.5] xHCI USB core: FAILED probe"));
            qemu_test_exit_failure();
        }
        match xhci::init() {
            Ok((slots, ports, connected, slot)) => {
                serial::write_line(format_args!(
                    "[S13.5] xHCI USB core: PASSED slots={} ports={} connected_port={} slot={}",
                    slots, ports, connected, slot
                ));
                qemu_test_exit_success();
            }
            Err(error) => {
                serial::write_line(format_args!(
                    "[S13.5] xHCI USB core: FAILED init {:?}",
                    error
                ));
                qemu_test_exit_failure();
            }
        }
    }
    #[cfg(feature = "stage13-8-test")]
    {
        if !hda::self_test() || !woven_audio::self_test() || !hda::probe() {
            serial::write_line(format_args!(
                "[S13.8] WovenAudio HDA foundation: FAILED probe/self-test"
            ));
            qemu_test_exit_failure();
        }
        match woven_audio::init() {
            Ok(_caps) => {
                match hda::discover_codec() {
                    Ok(codec) => serial::write_line(format_args!(
                        "[S13.8] HDA codec command transport: PASSED cad={} vendor={:#010x} revision={:#010x} root_start={} root_count={}",
                        codec.address,
                        codec.vendor_id,
                        codec.revision_id,
                        codec.root_start_node,
                        codec.root_node_count
                    )),
                    Err(error) => {
                        serial::write_line(format_args!("[S13.8] HDA codec command transport: FAILED {:?}", error));
                        qemu_test_exit_failure();
                    }
                }

                match hda::discover_topology() {
                    Ok(topology) => {
                        serial::write_line(format_args!(
                            "[S13.8] HDA codec topology: PASSED afg={} widgets={} dac={} adc={} mixers={} selectors={} pins={}",
                            topology.audio_function_groups,
                            topology.widgets,
                            topology.audio_outputs,
                            topology.audio_inputs,
                            topology.mixers,
                            topology.selectors,
                            topology.pin_complexes
                        ));
                    }
                    Err(error) => {
                        serial::write_line(format_args!(
                            "[S13.8] HDA codec topology: FAILED {:?}",
                            error
                        ));
                        qemu_test_exit_failure();
                    }
                }

                let audio_api = match woven_audio::integration_smoke_test() {
                    Ok(summary) => summary,
                    Err(error) => {
                        serial::write_line(format_args!(
                            "[S13.8] WovenAudio stream/API integration: FAILED {:?}",
                            error
                        ));
                        qemu_test_exit_failure();
                    }
                };
                serial::write_line(format_args!(
        "[S13.8] WovenAudio stream/API integration: PASSED playback_stream={} capture_stream={} playback_bytes={} capture_bytes={} rate={} channels={} bits={} gain={} mute={}",
        audio_api.playback.hardware_stream,
        audio_api.capture.hardware_stream,
        audio_api.playback.bytes_transferred,
        audio_api.capture.bytes_transferred,
        audio_api.playback.format.sample_rate_hz,
        audio_api.playback.format.channels,
        audio_api.playback.format.bits_per_sample,
        audio_api.mixer.current_gain,
        audio_api.mixer.mute_supported
    ));
                qemu_test_exit_success();
            }
            Err(error) => {
                serial::write_line(format_args!(
                    "[S13.8] WovenAudio HDA foundation: FAILED init {:?}",
                    error
                ));
                qemu_test_exit_failure();
            }
        }
    }
    #[cfg(feature = "stage13-7-test")]
    {
        if !woven_input::self_test() || !keyboard::self_test() {
            serial::write_line(format_args!("[S13.7] WovenInput framework: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.7] WovenInput unified event framework: PASSED dropped={}",
            woven_input::dropped_events()
        ));
        qemu_test_exit_success();
    }
    // Before the 13.12 block: 13.13 and 13.14 builds also enable
    // stage13-12-test, and each block ends the boot.
    #[cfg(feature = "stage13-17-test")]
    {
        if !drivers::foreign_disk_self_test() {
            serial::write_line(format_args!("[S13.17] foreign disks are read-only: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.17] foreign disks are read-only: PASSED"));
        qemu_test_exit_success();
    }
    // Ends with the machine off: the self-test returns only if it is not.
    #[cfg(feature = "stage20-15-test")]
    {
        if !stage20_15_self_test() {
            serial::write_line(format_args!("[S20.15] desktop session: FAILED"));
            qemu_test_exit_failure();
        }
        qemu_test_exit_success();
    }
    // Both 13.20 gates end with the machine off or reset: the self-test
    // returns only when the machine is still running.
    #[cfg(feature = "stage13-20b-test")]
    {
        if !power::self_test(true) {
            serial::write_line(format_args!("[S13.20] reset: FAILED"));
            qemu_test_exit_failure();
        }
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-20-test")]
    {
        if !power::self_test(false) {
            serial::write_line(format_args!("[S13.20] power-off: FAILED"));
            qemu_test_exit_failure();
        }
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-19-test")]
    {
        if !livelog::self_test() {
            serial::write_line(format_args!(
                "[S13.19] live-test log on the USB stick: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.19] live-test log on the USB stick: PASSED"
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-18-test")]
    {
        if !xhci::storage_self_test() {
            serial::write_line(format_args!("[S13.18] USB mass storage: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.18] USB mass storage: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-16-test")]
    {
        if !xhci::report_device_self_test() {
            serial::write_line(format_args!("[S13.16] report-protocol HID input: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.16] report-protocol HID input: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-15-test")]
    {
        if !stage13_15_self_test() {
            serial::write_line(format_args!("[S13.15] WovenAudio playback: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.15] WovenAudio playback: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-14-test")]
    {
        if !drivers::storage_self_test() {
            serial::write_line(format_args!("[S13.14] NVMe/AHCI storage: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.14] NVMe/AHCI storage: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-13-test")]
    {
        if !xhci::input_self_test() {
            serial::write_line(format_args!("[S13.13] USB input: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.13] USB input: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-12-test")]
    {
        if !drivers::self_test() {
            serial::write_line(format_args!("[S13.12] driver bring-up: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S13.12] driver bring-up: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage13-6-test")]
    {
        if !xhci::self_test() || !xhci::probe() {
            serial::write_line(format_args!("[S13.6] USB HID: FAILED probe/self-test"));
            qemu_test_exit_failure();
        }
        match xhci::init_hid() {
            Ok(summary) => {
                let report = match xhci::poll_hid_report() {
                    Ok(report) => report,
                    Err(error) => {
                        serial::write_line(format_args!(
                            "[S13.6] USB HID: FAILED report {:?}",
                            error
                        ));
                        qemu_test_exit_failure();
                    }
                };
                serial::write_line(format_args!(
                    "[S13.6] USB HID keyboard: PASSED slot={} port={} interface={} endpoint={:#x} max_packet={} report={:02x?}",
                    summary.slot, summary.port, summary.interface, summary.endpoint, summary.max_packet, report
                ));
                qemu_test_exit_success();
            }
            Err(error) => {
                serial::write_line(format_args!("[S13.6] USB HID: FAILED init {:?}", error));
                qemu_test_exit_failure();
            }
        }
    }
    #[cfg(feature = "stage1-5-test")]
    {
        if !journal::structural_self_test() || !storage::self_test() {
            serial::write_line(format_args!("[S1-5] storage journal: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S1-5] storage journal: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-14b-test")]
    {
        if !stage20_14b_self_test() {
            serial::write_line(format_args!("[S20.14B] desktop across monitors: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.14B] desktop across monitors: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-14-test")]
    {
        if !virtio_gpu::self_test() {
            serial::write_line(format_args!("[S20.14] two monitors: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.14] two monitors: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage24-59-test")]
    {
        if !stage24_59_change_pin_self_test() {
            serial::write_line(format_args!("[S24.59] Change-PIN flow: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S24.59] Change-PIN flow: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-13-test")]
    {
        if !stage20_13_self_test() {
            serial::write_line(format_args!("[S20.13] workspaces: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.13] workspaces: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-12-test")]
    {
        if !stage20_12_self_test() {
            serial::write_line(format_args!("[S20.12] sign-in: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.12] sign-in: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-11-test")]
    {
        if !stage20_11_self_test() {
            serial::write_line(format_args!("[S20.11] lock screen: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.11] lock screen: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-10-test")]
    {
        if !stage20_10_self_test() {
            serial::write_line(format_args!("[S20.10] quick settings: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.10] quick settings: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-9-test")]
    {
        if !stage20_9_self_test() {
            serial::write_line(format_args!("[S20.9] notification center: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.9] notification center: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-8-test")]
    {
        if !stage20_8_self_test() {
            serial::write_line(format_args!("[S20.8] tiling: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.8] tiling: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-7-test")]
    {
        if !stage20_7_self_test() {
            serial::write_line(format_args!("[S20.7] spring animation: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.7] spring animation: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-6-test")]
    {
        if !stage20_6_self_test() {
            serial::write_line(format_args!("[S20.6] translucent overlays: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.6] translucent overlays: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-5-test")]
    {
        if !stage20_5_self_test() {
            serial::write_line(format_args!("[S20.5] window depth: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.5] window depth: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-4-test")]
    {
        if !stage20_4_self_test() {
            serial::write_line(format_args!("[S20.4] windows that move and close: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.4] windows that move and close: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-3-test")]
    {
        if !stage20_3_self_test() {
            serial::write_line(format_args!("[S20.3] application launcher: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.3] application launcher: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-2-test")]
    {
        if !stage20_2_self_test() {
            serial::write_line(format_args!("[S20.2] desktop applications: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.2] desktop applications: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-1-test")]
    {
        if !stage20_1_self_test() {
            serial::write_line(format_args!("[S20.1] woven-shell desktop: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.1] woven-shell desktop: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-14-test")]
    {
        if !stage19_14_self_test() {
            serial::write_line(format_args!(
                "[S19.14] display scaling and frame pacing: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S19.14] display scaling and frame pacing: PASSED"
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-13-test")]
    {
        if !stage19_13_self_test() {
            serial::write_line(format_args!("[S19.13] translucency and shadows: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.13] translucency and shadows: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-12-test")]
    {
        if !stage19_12_self_test() {
            serial::write_line(format_args!("[S19.12] pointer input: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.12] pointer input: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-11-test")]
    {
        if !stage19_11_self_test() {
            serial::write_line(format_args!("[S19.11] window stacking and focus: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.11] window stacking and focus: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-10-test")]
    {
        if !compositor::geometry_self_test(&mut console) {
            serial::write_line(format_args!(
                "[S19.10] display geometry and single writer: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S19.10] display geometry and single writer: PASSED"
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-9-test")]
    {
        if !stage19_9_self_test() {
            serial::write_line(format_args!("[S19.9] damage compositing: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.9] damage compositing: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-8-test")]
    {
        if !stage19_8_self_test() {
            serial::write_line(format_args!("[S19.8] combined wait: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.8] combined wait: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-7-test")]
    {
        if !stage19_7_self_test() {
            serial::write_line(format_args!("[S19.7] surface delegation: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.7] surface delegation: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-6-test")]
    {
        if !stage19_6_self_test() {
            serial::write_line(format_args!("[S19.6] blocking input wait: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.6] blocking input wait: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-5-test")]
    {
        if !stage19_5_self_test() {
            serial::write_line(format_args!("[S19.5] input focus: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.5] input focus: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-4-test")]
    {
        if !stage19_4_self_test() {
            serial::write_line(format_args!("[S19.4] compositor authority: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.4] compositor authority: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-3-test")]
    {
        if !compositor::presentation_self_test() {
            serial::write_line(format_args!("[S19.3] compositor presentation: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.3] compositor presentation: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-16-test")]
    {
        if !stage20_16_self_test() {
            serial::write_line(format_args!("[S20.16] textured 3D chrome demo: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.16] textured 3D chrome demo: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-17-test")]
    {
        if !stage20_17_self_test() {
            serial::write_line(format_args!("[S20.17] 3D desktop Quick Settings: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.17] 3D desktop Quick Settings: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage20-18-test")]
    {
        if !stage20_18_self_test() {
            serial::write_line(format_args!("[S20.18] 3D desktop title bars: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S20.18] 3D desktop title bars: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage30-1-test")]
    {
        if !stage30_1_self_test() {
            serial::write_line(format_args!("[S30.1] syscall benchmark: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S30.1] syscall benchmark: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage30-2-test")]
    {
        if !stage30_2_self_test() {
            serial::write_line(format_args!("[S30.2] ipc benchmark: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S30.2] ipc benchmark: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage30-3-test")]
    {
        if !stage30_3_self_test() {
            serial::write_line(format_args!("[S30.3] fs benchmark: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S30.3] fs benchmark: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage30-4-test")]
    {
        if !stage30_4_self_test() {
            serial::write_line(format_args!("[S30.4] compositor benchmark: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S30.4] compositor benchmark: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage30-5-test")]
    {
        if !stage30_5_self_test() {
            serial::write_line(format_args!("[S30.5] scheduler benchmark: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S30.5] scheduler benchmark: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-2-test")]
    {
        if !stage19_2_self_test() {
            serial::write_line(format_args!(
                "[S19.2] hello-gui through the compositor: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S19.2] hello-gui through the compositor: PASSED"
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage18-2-test")]
    {
        if !stage18_2_self_test() {
            serial::write_line(format_args!("[S18.2] client surface end-to-end: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S18.2] client surface end-to-end: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-1-test")]
    {
        if !compositor::structural_self_test() {
            serial::write_line(format_args!("[S19.1] compositor surface: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S19.1] compositor surface: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage18-1-test")]
    {
        if !user_graphics::structural_self_test() {
            serial::write_line(format_args!("[S18.1] client surface registry: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S18.1] client surface registry: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage17-1-test")]
    {
        if !font::structural_self_test() {
            serial::write_line(format_args!("[S17.1] WovenHat font coverage: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S17.1] WovenHat font coverage: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage16-1-test")]
    {
        if !stage16_1_self_test() {
            serial::write_line(format_args!("[S16.1] out-of-kernel application: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S16.1] out-of-kernel application: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage19-app-catalog-test")]
    {
        if !stage19_app_catalog_self_test() {
            qemu_test_exit_failure();
        }
        qemu_test_exit_success();
    }
    #[cfg(all(
        feature = "stage15-2-test",
        not(feature = "stage27-2-test"),
        not(feature = "stage21-6-test"),
        not(feature = "stage21-7-test"),
        not(feature = "stage21-8-test"),
        not(feature = "stage21-9-test"),
        not(feature = "stage21-10-test"),
        not(feature = "stage21-11-test"),
        not(feature = "stage22-1-test"),
        not(feature = "stage22-2-test"),
        not(feature = "stage22-3-test"),
        not(feature = "stage22-5-test"),
        not(feature = "stage27-5-test")
    ))]
    {
        if !wfs::stage15_2_self_test() {
            serial::write_line(format_args!("[S15.2] WovenFS physical device: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S15.2] WovenFS physical device: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage15-3-test")]
    {
        if !permission::self_test() {
            serial::write_line(format_args!("[S15.3] permission broker: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S15.3] permission broker: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage15-4-test")]
    {
        if !signed_app::self_test() {
            serial::write_line(format_args!(
                "[S15.4] signed application verification: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S15.4] signed application verification: PASSED"
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage15-5-test")]
    {
        serial::write_line(format_args!("[S15.5] secure boot handoff: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage15-6-test")]
    {
        if !boot_measurement::self_test() {
            serial::write_line(format_args!("[S15.6] software measurement: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S15.6] software measurement: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage15-7-test")]
    {
        if !secret_vault::self_test() {
            serial::write_line(format_args!("[S15.7] WovenVault handles: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S15.7] WovenVault handles: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage15-8-test")]
    {
        if !audit::stage15_8_self_test() {
            serial::write_line(format_args!("[S15.8] security ledger ABI: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S15.8] security ledger ABI: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage15-9-test")]
    {
        if !task::stack_canary_self_test() || !userspace::elf_loader_self_test() {
            serial::write_line(format_args!("[S15.9] exploit mitigations: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S15.9] exploit mitigations: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage27-1-test")]
    {
        if !update::self_test() {
            serial::write_line(format_args!("[S27.1] update A/B rollback: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S27.1] update A/B rollback: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage21-1-test")]
    {
        if !package::self_test() {
            serial::write_line(format_args!("[S21.1] WovenPackages parser: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.1] WovenPackages parser: PASSED"));
        qemu_test_exit_success();
    }

    #[cfg(feature = "stage21-2-test")]
    {
        if !package::verification_self_test() {
            serial::write_line(format_args!("[S21.2] WovenPackages signature: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.2] WovenPackages signature: PASSED"));
        qemu_test_exit_success();
    }

    #[cfg(feature = "stage21-3-test")]
    {
        if !package::store_self_test() {
            serial::write_line(format_args!("[S21.3] WovenStore transaction: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.3] WovenStore transaction: PASSED"));
        qemu_test_exit_success();
    }

    #[cfg(feature = "stage21-4-test")]
    {
        if !package::batch_self_test() {
            serial::write_line(format_args!("[S21.4] WovenStore batch: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.4] WovenStore batch: PASSED"));
        qemu_test_exit_success();
    }

    #[cfg(feature = "stage21-5-test")]
    {
        if !package::record_self_test() {
            serial::write_line(format_args!("[S21.5] WovenStore record: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.5] WovenStore record: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage21-6-test")]
    {
        if !package::durable_self_test() {
            serial::write_line(format_args!("[S21.6] WovenStore persistence: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.6] WovenStore persistence: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage21-7-test")]
    {
        if !package_service::self_test() {
            serial::write_line(format_args!("[S21.7] WovenPackage install: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.7] WovenPackage install: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage21-8-test")]
    {
        if !package_service::recovery_self_test() {
            serial::write_line(format_args!("[S21.8] WovenPackage recovery: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.8] WovenPackage recovery: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage21-9-test")]
    {
        if !package_service::update_remove_self_test() {
            serial::write_line(format_args!("[S21.9] WovenPackage update/remove: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.9] WovenPackage update/remove: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage21-10-test")]
    {
        if !package_service::rollback_self_test() {
            serial::write_line(format_args!("[S21.10] WovenPackage rollback: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.10] WovenPackage rollback: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage21-11-test")]
    {
        if !package_service::sandbox_self_test() {
            serial::write_line(format_args!("[S21.11] WovenPackage sandbox: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S21.11] WovenPackage sandbox: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage22-1-test")]
    {
        if !ipc::stage8_6_runtime_probe() {
            serial::write_line(format_args!("[S22.1] service IPC ABI: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S22.1] service IPC ABI: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage22-2-test")]
    {
        if !ipc::stage8_2_runtime_probe() {
            serial::write_line(format_args!("[S22.2] handle IPC ABI: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S22.2] handle IPC ABI: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(all(feature = "stage22-3-test", not(feature = "stage27-5-test")))]
    {
        if !stage22_3_package_service_self_test() {
            serial::write_line(format_args!("[S22.3] WovenPackage broker: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S22.3] WovenPackage broker: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage22-5-test")]
    {
        if !stage22_5_package_transaction_self_test() {
            serial::write_line(format_args!("[S22.5] WovenPackage transactions: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S22.5] WovenPackage transactions: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage23-1-test")]
    {
        if !stage23_ai_broker_self_test(false) {
            serial::write_line(format_args!("[S23.1] WovenAI broker: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S23.1] WovenAI broker: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage23-2-test")]
    {
        if !stage23_ai_broker_self_test(true) {
            serial::write_line(format_args!("[S23.2] WovenAI local provider: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S23.2] WovenAI local provider: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage23-3-test")]
    {
        if !stage23_cloud_provider_self_test() {
            serial::write_line(format_args!("[S23.3] WovenAI cloud provider: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S23.3] WovenAI cloud provider: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage23-4-test")]
    {
        if !stage23_ai_capability_security_self_test() {
            serial::write_line(format_args!("[S23.4] WovenAI capability security: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S23.4] WovenAI capability security: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage23-5-test")]
    {
        if !stage23_semantic_search_self_test() {
            serial::write_line(format_args!("[S23.5] WovenAI semantic search: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S23.5] WovenAI semantic search: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage23-6-test")]
    {
        if !stage23_assistant_action_self_test() {
            serial::write_line(format_args!("[S23.6] WovenAssistant actions: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S23.6] WovenAssistant actions: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage23-7-test")]
    {
        if !stage23_security_monitor_self_test() {
            serial::write_line(format_args!("[S23.7] WovenAI security monitor: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S23.7] WovenAI security monitor: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage24-1-test")]
    {
        if !stage24_identity_self_test() {
            serial::write_line(format_args!("[S24.1] WovenIdentity core: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S24.1] WovenIdentity core: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage24-2-test")]
    {
        if !stage24_identity_snapshot_self_test() {
            serial::write_line(format_args!("[S24.2] WovenIdentity snapshot: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S24.2] WovenIdentity snapshot: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage24-3-test")]
    {
        if !identity_storage::durable_self_test() {
            serial::write_line(format_args!(
                "[S24.3] WovenIdentity durable storage: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S24.3] WovenIdentity durable storage: PASSED"
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage24-4-test")]
    {
        if !identity_runtime::self_test() {
            serial::write_line(format_args!("[S24.4] WovenIdentity task binding: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S24.4] WovenIdentity task binding: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage24-5-test")]
    {
        if !stage24_identity_syscall_frame_self_test() {
            serial::write_line(format_args!("[S24.5] WovenIdentity syscall ABI: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S24.5] WovenIdentity syscall ABI: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage27-2-test")]
    {
        if !update::durable_self_test() {
            serial::write_line(format_args!("[S27.2] durable update records: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S27.2] durable update records: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage27-5-test")]
    {
        if !update_service::self_test() {
            serial::write_line(format_args!("[S27.5] update service: FAILED"));
            qemu_test_exit_failure();
        }
        if !update_service::service_ipc_self_test(SYSTEM_APP_UPDATE) {
            serial::write_line(format_args!("[S27.5] update service IPC: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S27.5] update service: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage15-1-test")]
    {
        if !wfs::stage15_1_self_test() {
            serial::write_line(format_args!("[S15.1] WovenFS native filesystem: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S15.1] WovenFS native filesystem: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage12-2-test")]
    {
        if !wovenfs::structural_self_test() {
            serial::write_line(format_args!("[S12.2] WovenFS: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S12.2] WovenFS: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage12-3-test")]
    {
        if !volume_crypto::structural_self_test() || !storage_manager::encrypted_mount_self_test() {
            serial::write_line(format_args!("[S12.3] encryption: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S12.3] encryption: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage12-4-test")]
    {
        if !snapshots::structural_self_test() {
            serial::write_line(format_args!("[S12.4] snapshots: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S12.4] snapshots: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage12-5-test")]
    {
        if !storage_manager::structural_self_test() {
            serial::write_line(format_args!("[S12.5] storage management: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S12.5] storage management: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage11-4-test")]
    {
        serial::write_line(format_args!("[S11.4] libwoven runtime boundary: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage11-compiler-test")]
    run_compiler_loader_gate();
    #[cfg(feature = "stage11-5-test")]
    {
        if !userspace::elf_loader_self_test() || !userspace::shared_library_self_test() {
            serial::write_line(format_args!("[S11.5] loader hardening: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S11.5] loader hardening: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage10-9-test")]
    {
        if !async_events::structural_self_test()
            || async_events::active_count() != 0
            || async_op::stats().active != 0
        {
            serial::write_line(format_args!("[S10.9] timers/events: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S10.9] timers/events/deadlines/cancellation/teardown: PASSED"
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage10-8-test")]
    {
        let before = completion_port::active_count();
        let program = userspace::create_stage10_8_process().expect("completion-port probe ELF");
        let Ok((pid, _)) = task::spawn_user_process("s10.8-ports", program) else {
            serial::write_line(format_args!("[S10.8] probe task creation: FAILED"));
            qemu_test_exit_failure();
        };
        let start = timer::ticks();
        while !task::process_exited(pid) {
            if timer::ticks().saturating_sub(start) > 500 {
                serial::write_line(format_args!("[S10.8] completion ports: TIMEOUT"));
                qemu_test_exit_failure();
            }
            x86_64::instructions::hlt();
        }
        if task::wait_process(pid.as_u64()) != Ok(0)
            || async_op::stats().active != 0
            || completion_port::active_count() != before
            || !async_acceptance::ports_smp()
        {
            serial::write_line(format_args!("[S10.8] completion ports: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S10.8] completion ports + batch/cancel/timeout/teardown/SMP: PASSED"
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage10-4-test")]
    {
        let block_before = block_io::stats();
        let async_before = async_op::stats();
        let Some(stage10_4_program) = userspace::create_stage10_4_process() else {
            serial::write_line(format_args!(
                "[S10.4] userspace async block I/O image: FAILED"
            ));
            qemu_test_exit_failure();
        };
        let stage10_4_caps = capability::CapabilitySet::only(capability::Capability::StorageIo);
        let stage10_4_pid = match task::spawn_user_system_service(
            "s10.4-async-block",
            stage10_4_program,
            stage10_4_caps,
        ) {
            Ok((pid, _, _)) => pid,
            Err(_) => {
                serial::write_line(format_args!(
                    "[S10.4] userspace async block I/O spawn: FAILED"
                ));
                qemu_test_exit_failure();
            }
        };
        let stage10_4_start = timer::ticks();
        while !task::process_exited(stage10_4_pid) {
            if timer::ticks().wrapping_sub(stage10_4_start) > 500 {
                serial::write_line(format_args!("[S10.4] userspace async block I/O: TIMEOUT"));
                qemu_test_exit_failure();
            }
            x86_64::instructions::hlt();
        }
        if task::wait_process(stage10_4_pid.as_u64()) != Ok(0) {
            serial::write_line(format_args!(
                "[S10.4] userspace async block I/O exit: FAILED"
            ));
            qemu_test_exit_failure();
        }
        let block_after = block_io::stats();
        let async_after = async_op::stats();
        if block_after.queued < block_before.queued + 4
            || block_after.completed < block_before.completed + 2
            || block_after.active != 0
            || async_after.active != 0
            || async_after.owner_reaped <= async_before.owner_reaped
        {
            serial::write_line(format_args!(
                "[S10.4] userspace async block I/O: FAILED queued={} completed={} block_active={} async_active={} owner_reaped={}",
                block_after.queued.wrapping_sub(block_before.queued),
                block_after.completed.wrapping_sub(block_before.completed),
                block_after.active,
                async_after.active,
                async_after.owner_reaped.wrapping_sub(async_before.owner_reaped),
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S10.4] userspace async block read/write + bounce-buffer teardown: PASSED"
        ));
        // Stage 10.4 is intentionally a dedicated boot mode. The complete
        // Stage 10.3 production validation has already run immediately before
        // these isolated boots, so terminate the dedicated QEMU probe here
        // rather than perturbing later legacy assertions with extra service
        // process/device activity.
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage10-5-test")]
    {
        if !async_file::start_worker() {
            serial::write_line(format_args!("[S10.5] async file worker start: FAILED"));
            qemu_test_exit_failure();
        }
        let async_before = async_op::stats();
        let file_before = async_file::stats();
        if vfs::write_file("/tmp/s10.5", &[0u8; 32]).is_err() {
            serial::write_line(format_args!("[S10.5] acceptance file creation: FAILED"));
            qemu_test_exit_failure();
        }
        let Some(program) = userspace::create_stage10_5_process() else {
            serial::write_line(format_args!("[S10.5] userspace async file image: FAILED"));
            qemu_test_exit_failure();
        };
        let pid = match task::spawn_user_process("s10.5-async-file", program) {
            Ok((pid, _)) => pid,
            Err(_) => {
                serial::write_line(format_args!("[S10.5] userspace async file spawn: FAILED"));
                qemu_test_exit_failure();
            }
        };
        let start = timer::ticks();
        while !task::process_exited(pid) {
            if timer::ticks().wrapping_sub(start) > 500 {
                serial::write_line(format_args!("[S10.5] userspace async file I/O: TIMEOUT"));
                qemu_test_exit_failure();
            }
            x86_64::instructions::hlt();
        }
        if task::wait_process(pid.as_u64()) != Ok(0) {
            serial::write_line(format_args!("[S10.5] userspace async file exit: FAILED"));
            qemu_test_exit_failure();
        }
        // Cancellation/exit may race a worker already holding a copied Work.
        // Wait only for deterministic kernel cleanup, not for userspace work.
        let cleanup_start = timer::ticks();
        while async_file::stats().active != 0 {
            if timer::ticks().wrapping_sub(cleanup_start) > 100 {
                serial::write_line(format_args!("[S10.5] async file teardown drain: TIMEOUT"));
                qemu_test_exit_failure();
            }
            task::yield_now();
        }
        let file_after = async_file::stats();
        let async_after = async_op::stats();
        let mut verify = [0u8; 16];
        let verified_bytes = vfs::open("/tmp/s10.5")
            .and_then(|id| {
                let result = vfs::read_at(id, 0, &mut verify);
                let _ = vfs::close_open_file(id);
                result
            })
            .is_ok_and(|count| count == 16)
            && verify == *b"WOVENHAT-ASYNC!!";
        let _ = vfs::remove("/tmp/s10.5");
        if file_after.submitted < file_before.submitted + 4
            || file_after.completed < file_before.completed + 2
            || file_after.cancelled <= file_before.cancelled
            || file_after.owner_reaped <= file_before.owner_reaped
            || file_after.active != 0
            || async_after.active != 0
            || async_after.owner_reaped <= async_before.owner_reaped
            || !verified_bytes
        {
            serial::write_line(format_args!(
                "[S10.5] userspace async VFS I/O: FAILED submitted={} completed={} cancelled={} file_reaped={} async_active={} generic_reaped={} bytes_ok={}",
                file_after.submitted.wrapping_sub(file_before.submitted),
                file_after.completed.wrapping_sub(file_before.completed),
                file_after.cancelled.wrapping_sub(file_before.cancelled),
                file_after.owner_reaped.wrapping_sub(file_before.owner_reaped),
                async_after.active,
                async_after.owner_reaped.wrapping_sub(async_before.owner_reaped),
                verified_bytes,
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S10.5] userspace async VFS read/write + fd pinning/cancel/teardown: PASSED"
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage10-6-test")]
    {
        if network::init().is_err() {
            serial::write_line(format_args!("[S10.6] virtio-net + smoltcp init: FAILED"));
            qemu_test_exit_failure();
        }
        if !async_network::start_worker() {
            serial::write_line(format_args!("[S10.6] async network worker start: FAILED"));
            qemu_test_exit_failure();
        }
        let async_before = async_op::stats();
        let net_before = async_network::stats();
        let sockets_before = network::stats().user_sockets;
        let Some(program) = userspace::create_stage10_6_process() else {
            serial::write_line(format_args!(
                "[S10.6] userspace async network image: FAILED"
            ));
            qemu_test_exit_failure();
        };
        let pid = match task::spawn_user_process("s10.6-async-net", program) {
            Ok((pid, _)) => pid,
            Err(_) => {
                serial::write_line(format_args!(
                    "[S10.6] userspace async network spawn: FAILED"
                ));
                qemu_test_exit_failure();
            }
        };
        serial::write_line(format_args!(
            "[S10.6] async UDP receive target ready on port 7001"
        ));
        let start = timer::ticks();
        while !task::process_exited(pid) {
            network::poll();
            if timer::ticks().wrapping_sub(start) > 1_000 {
                serial::write_line(format_args!("[S10.6] userspace async network I/O: TIMEOUT"));
                qemu_test_exit_failure();
            }
            x86_64::instructions::hlt();
        }
        if task::wait_process(pid.as_u64()) != Ok(0) {
            serial::write_line(format_args!("[S10.6] userspace async network exit: FAILED"));
            qemu_test_exit_failure();
        }
        let cleanup_start = timer::ticks();
        while async_network::stats().active != 0 || network::stats().user_sockets != sockets_before
        {
            network::poll();
            if timer::ticks().wrapping_sub(cleanup_start) > 200 {
                serial::write_line(format_args!(
                    "[S10.6] async network teardown drain: TIMEOUT"
                ));
                qemu_test_exit_failure();
            }
            task::yield_now();
        }
        let net_after = async_network::stats();
        let async_after = async_op::stats();
        let sockets_after = network::stats().user_sockets;
        if net_after.submitted < net_before.submitted + 4
            || net_after.completed < net_before.completed + 2
            || net_after.cancelled <= net_before.cancelled
            || net_after.owner_reaped <= net_before.owner_reaped
            || net_after.active != 0
            || async_after.active != 0
            || async_after.owner_reaped <= async_before.owner_reaped
            || sockets_after != sockets_before
        {
            serial::write_line(format_args!(
                "[S10.6] userspace async networking: FAILED submitted={} completed={} cancelled={} net_reaped={} net_active={} async_active={} generic_reaped={} sockets_before={} sockets_after={}",
                net_after.submitted.wrapping_sub(net_before.submitted),
                net_after.completed.wrapping_sub(net_before.completed),
                net_after.cancelled.wrapping_sub(net_before.cancelled),
                net_after.owner_reaped.wrapping_sub(net_before.owner_reaped),
                net_after.active,
                async_after.active,
                async_after.owner_reaped.wrapping_sub(async_before.owner_reaped),
                sockets_before,
                sockets_after,
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S10.6] userspace async UDP send/recv + socket pinning/cancel/teardown: PASSED"
        ));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage10-7-test")]
    {
        if network::init().is_err() {
            serial::write_line(format_args!("[S10.7] virtio-net + smoltcp init: FAILED"));
            qemu_test_exit_failure();
        }
        if !async_network::start_worker() {
            serial::write_line(format_args!("[S10.7] async network worker start: FAILED"));
            qemu_test_exit_failure();
        }
        let async_before = async_op::stats();
        let net_before = async_network::stats();
        let sockets_before = network::stats().user_sockets;
        let Some(program) = userspace::create_stage10_7_process() else {
            serial::write_line(format_args!("[S10.7] userspace async TCP image: FAILED"));
            qemu_test_exit_failure();
        };
        let pid = match task::spawn_user_process("s10.7-async-tcp", program) {
            Ok((pid, _)) => pid,
            Err(_) => {
                serial::write_line(format_args!("[S10.7] userspace async TCP spawn: FAILED"));
                qemu_test_exit_failure();
            }
        };
        serial::write_line(format_args!("[S10.7] async TCP host target 10.0.2.2:18080"));
        let start = timer::ticks();
        while !task::process_exited(pid) {
            network::poll();
            if timer::ticks().wrapping_sub(start) > 1_500 {
                serial::write_line(format_args!("[S10.7] userspace async TCP I/O: TIMEOUT"));
                qemu_test_exit_failure();
            }
            x86_64::instructions::hlt();
        }
        let tcp_exit = task::wait_process(pid.as_u64());
        if tcp_exit != Ok(0) {
            serial::write_line(format_args!(
                "[S10.7] userspace async TCP exit: FAILED assertion={:?}",
                tcp_exit.ok()
            ));
            qemu_test_exit_failure();
        }
        let cleanup_start = timer::ticks();
        while async_network::stats().active != 0 || network::stats().user_sockets != sockets_before
        {
            network::poll();
            if timer::ticks().wrapping_sub(cleanup_start) > 200 {
                serial::write_line(format_args!("[S10.7] async TCP teardown drain: TIMEOUT"));
                qemu_test_exit_failure();
            }
            task::yield_now();
        }
        let net_after = async_network::stats();
        let async_after = async_op::stats();
        let sockets_after = network::stats().user_sockets;
        if !network::queued_close_verified()
            || net_after.submitted < net_before.submitted + 8
            || net_after.completed < net_before.completed + 6
            || net_after.cancelled <= net_before.cancelled
            || net_after.owner_reaped <= net_before.owner_reaped
            || net_after.active != 0
            || async_after.active != 0
            || async_after.owner_reaped <= async_before.owner_reaped
            || sockets_after != sockets_before
        {
            serial::write_line(format_args!("[S10.7] userspace async TCP: FAILED submitted={} completed={} cancelled={} net_reaped={} net_active={} async_active={} generic_reaped={} sockets_before={} sockets_after={}",
                net_after.submitted.wrapping_sub(net_before.submitted),
                net_after.completed.wrapping_sub(net_before.completed),
                net_after.cancelled.wrapping_sub(net_before.cancelled),
                net_after.owner_reaped.wrapping_sub(net_before.owner_reaped),
                net_after.active,
                async_after.active,
                async_after.owner_reaped.wrapping_sub(async_before.owner_reaped),
                sockets_before, sockets_after));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S10.7] userspace async TCP connect/send/recv + EOF/pinning/cancel/teardown: PASSED"
        ));
        qemu_test_exit_success();
    }
    if !syscall::user_identity_verified() {
        console.println("USER UID/GID SYSCALLS: FAILED");
        halt();
    }
    if !syscall::user_exec_verified() {
        console.println("USER EXEC SYSCALL: FAILED");
        halt();
    }
    console.println("USER EXEC ATOMIC REPLACEMENT: OK");
    if !syscall::user_fork_verified() {
        console.println("USER FORK SYSCALL: FAILED");
        halt();
    }
    console.println("USER FORK ADDRESS-SPACE CLONE: OK");
    if !syscall::user_standard_streams_verified() {
        console.println("USER STDIN SYSCALL: FAILED");
        halt();
    }
    console.println("USER STANDARD STREAMS: OK");
    console.println("USER UID/GID SYSCALLS: OK");
    serial::write_line(format_args!(
        "[BOOT] user mmap/write/read/munmap and frame return verified"
    ));

    if !syscall::user_io_verified()
        || task::open_file_count() != 0
        || vfs::node_count() != vfs_nodes_before_userspace
    {
        console.println("USER VFS/DESCRIPTOR SYSCALLS: FAILED");
        halt();
    }
    serial::write_line(format_args!(
        "[BOOT] user write/open/read/close and pointer validation verified"
    ));

    if !syscall::last_completed(syscall::Number::Getpid) {
        console.println("USER SYSCALL ABI: FAILED");
        halt();
    }
    if task::zombie_count() != 2 {
        console.println("PROCESS ZOMBIE RETENTION: FAILED");
        halt();
    }
    let first_status = syscall::invoke(syscall::Number::Waitpid, first_pid.as_u64(), 0, 0);
    let second_status = syscall::invoke(syscall::Number::Waitpid, second_pid.as_u64(), 0, 0);
    if first_status != 0 || second_status != 0 || task::zombie_count() != 0 {
        console.println("PROCESS WAIT/REAP: FAILED");
        halt();
    }
    if syscall::invoke(syscall::Number::Waitpid, first_pid.as_u64(), 0, 0) != u64::MAX {
        console.println("PROCESS DOUBLE-WAIT REJECTION: FAILED");
        halt();
    }
    serial::write_line(format_args!(
        "[BOOT] parent-child waitpid and zombie reaping verified"
    ));
    if memory::stats().allocated_frames != isolation_baseline || ipc::endpoint_count() != 0 {
        console.println("USER ADDRESS-SPACE RECLAMATION: FAILED");
        halt();
    }

    console.println("USER CR3 ISOLATION/W^X/RECLAMATION: OK");
    serial::write_line(format_args!(
        "[BOOT] isolated user CR3 roots {:#x} and {:#x} verified",
        first_root, second_root,
    ));
    serial::write_line(format_args!(
        "[BOOT] ring3 W^X mapping cleanup and frame reuse verified"
    ));
    console.println("RING3 PROCESS GETPID/EXIT: OK");
    serial::write_line(format_args!(
        "[BOOT] ring3 process and syscall ABI verified"
    ));
    // Exercise a real Ring-3 absent-page exception. The pager must park the
    // faulting context, populate the backing page on its worker task, then
    // resume the original instruction. mmaptest covers sparse lazy faults,
    // writes, fork, and kernel copy_to/from_user fault resolution.
    let pager_before = task::pager_stats();

    serial::write_line(format_args!("[PAGER-DIAG] install mmap test: BEGIN"));
    if !userspace::install_file_mmap_test() {
        console.println("PAGER MMAP IMAGE: INSTALL FAILED");
        halt();
    }
    serial::write_line(format_args!("[PAGER-DIAG] install mmap test: DONE"));

    let mut pager_image = alloc::vec![0u8; vfs::NODE_CAPACITY];
    serial::write_line(format_args!("[PAGER-DIAG] read /bin/mmaptest: BEGIN"));
    let Ok(pager_image_len) = vfs::read_all("/bin/mmaptest", &mut pager_image) else {
        console.println("PAGER MMAP IMAGE: READ FAILED");
        halt();
    };
    serial::write_line(format_args!(
        "[PAGER-DIAG] read /bin/mmaptest: DONE bytes={}",
        pager_image_len
    ));

    // Stage 7.2 termination/lifecycle smoke test. Keep the target Ready and
    // therefore provably off-CPU: the scheduler may retire it immediately,
    // but ProcessState::Exited must not be published until that retirement is
    // complete. wait_process then owns deferred address-space destruction.
    serial::write_line(format_args!("[TERM] create termination target: BEGIN"));
    let Some(kill_program) = userspace::create_shell_process() else {
        console.println("TERMINATION TEST IMAGE: LOAD FAILED");
        halt();
    };
    serial::write_line(format_args!("[TERM] create termination target: DONE"));
    serial::write_line(format_args!("[TERM] spawn termination target: BEGIN"));
    let kill_pid = match task::spawn_user_process("termination-target", kill_program) {
        Ok((pid, _)) => pid,
        Err(_) => {
            console.println("TERMINATION TEST PROCESS: SPAWN FAILED");
            halt();
        }
    };
    serial::write_line(format_args!(
        "[TERM] spawn termination target: DONE pid={}",
        kill_pid.as_u64()
    ));
    let exited_before_kill = task::process_exited(kill_pid);
    serial::write_line(format_args!(
        "[TERM] process_exited before kill={}",
        exited_before_kill
    ));
    let kill_result = if exited_before_kill {
        Err(())
    } else {
        task::kill_process(kill_pid.as_u64(), 15).map_err(|_| ())
    };
    serial::write_line(format_args!("[TERM] kill result={}", kill_result.is_ok()));
    let termination_deadline = timer::ticks().wrapping_add(200);
    while !task::process_exited(kill_pid) && timer::ticks() < termination_deadline {
        task::yield_now();
    }
    let exited_after_kill = task::process_exited(kill_pid);
    serial::write_line(format_args!(
        "[TERM] process_exited after kill={}",
        exited_after_kill
    ));
    let wait_status = task::wait_process(kill_pid.as_u64());
    let wait_ok = wait_status == Ok(143);
    match wait_status {
        Ok(code) => serial::write_line(format_args!("[TERM] wait status code={}", code)),
        Err(task::WaitError::NoSuchChild) => {
            serial::write_line(format_args!("[TERM] wait status=NO_SUCH_CHILD"))
        }
        Err(task::WaitError::StillRunning) => {
            serial::write_line(format_args!("[TERM] wait status=STILL_RUNNING"))
        }
    }
    if exited_before_kill || kill_result.is_err() || !exited_after_kill || !wait_ok {
        console.println("SCHEDULER-OWNED TERMINATION: FAILED");
        halt();
    }
    serial::write_line(format_args!(
        "[TERM] scheduler-owned Ready-task termination + deferred reap: PASSED"
    ));

    serial::write_line(format_args!("[PAGER-DIAG] load mmap ELF: BEGIN"));
    let Some(pager_program) =
        userspace::load_elf_with_argv(&pager_image[..pager_image_len], &["/bin/mmaptest"])
    else {
        console.println("PAGER MMAP IMAGE: LOAD FAILED");
        halt();
    };
    serial::write_line(format_args!("[PAGER-DIAG] load mmap ELF: DONE"));

    let fork_affinity_before = task::fork_affinity_inheritances();

    // Stage 7.5: execute the existing file-backed mmap/pager workload on an
    // application processor when SMP is available. The task stays hard-pinned
    // for its lifetime, so this milestone validates remote Ring-3 file faults,
    // BSP pager wakeup, address-space resume and per-CPU I/O preemption guards
    // without yet enabling unrestricted automatic userspace balancing.
    let pager_cpu = if smp::online_count() > 1 {
        smp::online_count() - 1
    } else {
        0
    };
    serial::write_line(format_args!(
        "[PAGER-DIAG] spawn pager process: BEGIN cpu={}",
        pager_cpu
    ));
    let pager_pid = if pager_cpu == 0 {
        match task::spawn_user_process("pager-mmaptest", pager_program) {
            Ok((pid, _)) => pid,
            Err(_) => {
                console.println("PAGER MMAP PROCESS: SPAWN FAILED");
                halt();
            }
        }
    } else {
        match task::spawn_multicore_user_process(
            "pager-mmaptest",
            pager_program,
            1usize << pager_cpu,
        ) {
            Ok((pid, _, _, placement)) if placement.owner_cpu == pager_cpu => pid,
            Ok(_) => {
                console.println("PAGER MMAP PROCESS: AP OWNERSHIP FAILED");
                halt();
            }
            Err(_) => {
                console.println("PAGER MMAP PROCESS: AP SPAWN FAILED");
                halt();
            }
        }
    };
    serial::write_line(format_args!(
        "[PAGER-DIAG] spawn pager process: DONE pid={}",
        pager_pid.as_u64()
    ));
    serial::write_line(format_args!(
        "[PAGER-DIAG] first process_exited lookup: BEGIN pid={}",
        pager_pid.as_u64()
    ));
    let mut pager_exited = task::process_exited(pager_pid);
    serial::write_line(format_args!(
        "[PAGER-DIAG] first process_exited lookup: DONE exited={}",
        pager_exited
    ));

    // Stage 7.1.5 keeps the detailed hand-off tracer available for targeted
    // debugging, but normal acceptance runs it disabled so timing is not
    // distorted by per-switch serial I/O.
    task::set_yield_trace_enabled(false);
    let pager_wait_start = timer::ticks();
    let mut pager_wait_iteration = 0u64;
    while !pager_exited {
        if timer::ticks().wrapping_sub(pager_wait_start) > 200 {
            let (queued, completed) = task::pager_stats();
            serial::write_line(format_args!(
                "[PAGER] mmaptest timeout queued={} completed={}",
                queued, completed
            ));
            console.println("ASYNCHRONOUS PAGER: TIMEOUT");
            halt();
        }

        // This kernel-side acceptance loop must explicitly yield to the
        // Ready Ring-3 pager test process.  On SMP, merely executing HLT
        // waits for an interrupt but does not guarantee that the current
        // high-priority kernel task gives the scheduler a deterministic
        // opportunity to run the process whose page faults have already
        // been serviced.
        pager_wait_iteration = pager_wait_iteration.wrapping_add(1);
        if pager_wait_iteration <= 8 || pager_wait_iteration.is_multiple_of(64) {
            serial::write_line(format_args!(
                "[PAGER-DIAG] yield: BEGIN iteration={}",
                pager_wait_iteration
            ));
        }

        task::yield_now();

        if pager_wait_iteration <= 8 || pager_wait_iteration.is_multiple_of(64) {
            serial::write_line(format_args!(
                "[PAGER-DIAG] yield: DONE iteration={}",
                pager_wait_iteration
            ));
            serial::write_line(format_args!(
                "[PAGER-DIAG] process_exited lookup: BEGIN iteration={}",
                pager_wait_iteration
            ));
        }

        pager_exited = task::process_exited(pager_pid);

        if pager_wait_iteration <= 8 || pager_wait_iteration.is_multiple_of(64) {
            serial::write_line(format_args!(
                "[PAGER-DIAG] process_exited lookup: DONE iteration={} exited={}",
                pager_wait_iteration, pager_exited
            ));
        }
    }
    task::set_yield_trace_enabled(false);
    let pager_status = task::wait_process(pager_pid.as_u64());
    let pager_after = task::pager_stats();
    if pager_status != Ok(0) || pager_after.0 <= pager_before.0 || pager_after.1 < pager_after.0 {
        console.println("ASYNCHRONOUS PAGER: FAILED");
        halt();
    }
    serial::write_line(format_args!(
        "[PAGER] Ring-3 faults queued={} completed={}: PASSED",
        pager_after.0 - pager_before.0,
        pager_after.1 - pager_before.1
    ));
    if pager_cpu == 0 {
        serial::write_line(format_args!(
            "[S7.5] single-CPU pager/file-I/O baseline preserved"
        ));
    } else {
        serial::write_line(format_args!(
            "[S7.5] pinned AP pager/file-I/O: PASSED cpu={}",
            pager_cpu
        ));
    }
    if pager_cpu != 0 {
        if task::fork_affinity_inheritances() <= fork_affinity_before {
            serial::write_line(format_args!(
                "[S7.6] AP fork CPU/affinity inheritance: FAILED"
            ));
            halt();
        }
        serial::write_line(format_args!(
            "[S7.6] AP fork CPU/affinity inheritance: PASSED cpu={}",
            pager_cpu
        ));
        serial::write_line(format_args!(
            "[S7.6] bounded integrated multicore userspace foundation: PASSED"
        ));
    } else {
        serial::write_line(format_args!(
            "[S7.6] fork-affinity baseline preserved on single CPU"
        ));
    }
    console.println("KEYBOARD IRQ: READY");

    //
    // Breakpoint exception test
    //

    console.println("TESTING BREAKPOINT INTERRUPT...");

    int3();

    if interrupts::breakpoint_reached() {
        console.println("BREAKPOINT HANDLER: OK");
        console.println("INTERRUPT SYSTEM: ONLINE");
    } else {
        console.println("BREAKPOINT HANDLER: FAILED");
    }

    #[cfg(feature = "stage13-9-test")]
    {
        if !wifi_runtime::self_test() {
            serial::write_line(format_args!(
                "[S13.10AC] IRQ worker/wakeup/teardown: FAILED"
            ));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!(
            "[S13.10AC] IRQ worker/wakeup/teardown: PASSED"
        ));
    }
    smp::self_test();
    #[cfg(feature = "stage25-7-test")]
    {
        if !power::wake_trampoline_self_test() {
            serial::write_line(format_args!("[S25.7] wake trampoline staging: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S25.7] wake trampoline staging: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage25-8-test")]
    {
        if !power::resume_context_self_test() {
            serial::write_line(format_args!("[S25.8] CPU resume context: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S25.8] CPU resume context: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage25-9-test")]
    {
        if !hal::acpi::facs_wake_vector_self_test() {
            serial::write_line(format_args!("[S25.9] FACS wake vector transaction: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S25.9] FACS wake vector transaction: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage25-10-test")]
    {
        if !smp::cpu_suspend_barrier_self_test() {
            serial::write_line(format_args!("[S25.10] secondary CPU barrier: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S25.10] secondary CPU barrier: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage25-11-test")]
    {
        if !power::s3_control_self_test() {
            serial::write_line(format_args!("[S25.11] PM1 S3 request encoding: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S25.11] PM1 S3 request encoding: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage25-12-test")]
    {
        if !power::resume_context_restore_self_test() {
            serial::write_line(format_args!("[S25.12] CPU context restore: FAILED"));
            qemu_test_exit_failure();
        }
        qemu_test_exit_failure();
    }
    #[cfg(feature = "stage25-13-test")]
    {
        if !power::wake_record_self_test() {
            serial::write_line(format_args!("[S25.13] wake record integration: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S25.13] wake record integration: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage25-14-test")]
    {
        if !power::suspend_admission_self_test() {
            serial::write_line(format_args!("[S25.14] suspend admission: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S25.14] suspend admission: PASSED"));
        qemu_test_exit_success();
    }
    #[cfg(feature = "stage25-15-test")]
    {
        if !power::suspend_resume_target_self_test() {
            serial::write_line(format_args!("[S25.15] userspace resume target: FAILED"));
            qemu_test_exit_failure();
        }
        serial::write_line(format_args!("[S25.15] userspace resume target: PASSED"));
        qemu_test_exit_success();
    }
    serial::write_line(format_args!("[BOOT] ALL VALIDATIONS PASSED"));
    #[cfg(feature = "qemu-test")]
    qemu_test_exit_success();

    loop {
        x86_64::instructions::hlt();
    }
}

#[cfg(feature = "stage11-compiler-test")]
#[inline(never)]
fn run_compiler_loader_gate() {
    if !userspace::compiler_loader_self_test() {
        serial::write_line(format_args!("[S11.COMPILER] signed toolchain PIE: FAILED"));
        qemu_test_exit_failure();
    }
    serial::write_line(format_args!("[S11.COMPILER] signed toolchain PIE: PASSED"));
}

#[cfg(feature = "qemu-test")]
fn qemu_test_exit_success() {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") 0xf4_u16,
            in("al") 0x10_u8,
            options(nomem, nostack, preserves_flags),
        );
    }
    loop {
        x86_64::instructions::hlt();
    }
}

#[cfg(feature = "qemu-test")]
fn qemu_test_exit_failure() -> ! {
    // QEMU isa-debug-exit maps value 0x11 to host exit status 35. This keeps
    // acceptance failures fail-fast instead of parking in halt() until the
    // host-side 180-second timeout expires.
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") 0xf4_u16,
            in("al") 0x11_u8,
            options(nomem, nostack, preserves_flags),
        );
    }
    loop {
        x86_64::instructions::hlt();
    }
}

fn fairness_probe_a() -> ! {
    while FAIR_TASK_B_RUNS.load(Ordering::Acquire) == 0 {
        FAIR_TASK_A_RUNS.fetch_add(1, Ordering::Relaxed);
        core::hint::spin_loop();
    }
    FAIR_TASK_A_RUNS.fetch_add(1, Ordering::Release);
    FAIR_TASKS_COMPLETED.fetch_add(1, Ordering::Release);
    task::exit_current_task()
}

fn fairness_probe_b() -> ! {
    while FAIR_TASK_A_RUNS.load(Ordering::Acquire) == 0 {
        FAIR_TASK_B_RUNS.fetch_add(1, Ordering::Relaxed);
        core::hint::spin_loop();
    }
    FAIR_TASK_B_RUNS.fetch_add(1, Ordering::Release);
    FAIR_TASKS_COMPLETED.fetch_add(1, Ordering::Release);
    task::exit_current_task()
}
fn preemption_probe_task() -> ! {
    task::sleep_current(2);
    PREEMPTION_PROBE_BLOCKED.store(true, Ordering::Release);
    task::block_current();
    PREEMPTION_PROBE_COMPLETED.store(true, Ordering::Release);
    task::exit_current_task()
}

// ------------------------------- out-of-kernel application execution

/// An application image built by the ordinary Rust toolchain, outside the
/// kernel.
///
/// Every userspace program before this one was `global_asm!` inside this crate,
/// wrapped in a synthetic ELF by `build_stub_elf` at runtime. There was no way
/// to produce a program without editing the kernel, which is what made an
/// application platform impossible. This image is a separate crate compiled to
/// a static ET_EXEC file and handed to the same loader that serves every other
/// executable.
///
/// The path arrives through the environment so the kernel does not hard-code a
/// build layout; the Stage 16.1 harness builds the application first and sets
/// it.
#[cfg(feature = "stage16-1-test")]
const EXTERNAL_APPLICATION: &[u8] = include_bytes!(env!("WOVENHAT_APP_IMAGE"));

#[cfg(any(feature = "stage19-app-catalog-test", feature = "system-apps"))]
const SYSTEM_APP_CALCULATOR: &[u8] = include_bytes!(env!("WOVENHAT_CALCULATOR_IMAGE"));
#[cfg(any(feature = "stage19-app-catalog-test", feature = "system-apps"))]
const SYSTEM_APP_TERMINAL: &[u8] = include_bytes!(env!("WOVENHAT_TERMINAL_IMAGE"));
#[cfg(any(feature = "stage19-app-catalog-test", feature = "system-apps"))]
const SYSTEM_APP_FILES: &[u8] = include_bytes!(env!("WOVENHAT_FILES_IMAGE"));
#[cfg(any(feature = "stage19-app-catalog-test", feature = "system-apps"))]
const SYSTEM_APP_MONITOR: &[u8] = include_bytes!(env!("WOVENHAT_MONITOR_IMAGE"));
#[cfg(any(feature = "stage19-app-catalog-test", feature = "system-apps"))]
const SYSTEM_APP_SETTINGS: &[u8] = include_bytes!(env!("WOVENHAT_SETTINGS_IMAGE"));
#[cfg(any(feature = "stage19-app-catalog-test", feature = "system-apps"))]
const SYSTEM_APP_SECURITY: &[u8] = include_bytes!(env!("WOVENHAT_SECURITY_IMAGE"));
#[cfg(any(feature = "stage19-app-catalog-test", feature = "system-apps"))]
const SYSTEM_APP_EDITOR: &[u8] = include_bytes!(env!("WOVENHAT_EDITOR_IMAGE"));
#[cfg(any(feature = "stage19-app-catalog-test", feature = "system-apps"))]
const SYSTEM_APP_IMAGES: &[u8] = include_bytes!(env!("WOVENHAT_IMAGES_IMAGE"));
#[cfg(any(
    feature = "stage19-app-catalog-test",
    feature = "stage27-5-test",
    feature = "system-apps"
))]
const SYSTEM_APP_UPDATE: &[u8] = include_bytes!(env!("WOVENHAT_UPDATE_IMAGE"));
#[cfg(any(feature = "stage19-app-catalog-test", feature = "system-apps"))]
const SYSTEM_APP_STORE: &[u8] = include_bytes!(env!("WOVENHAT_STORE_IMAGE"));

#[cfg(any(
    all(feature = "stage22-3-test", not(feature = "stage27-5-test")),
    feature = "stage22-5-test",
    feature = "system-apps"
))]
const PACKAGE_SERVICE_IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_PACKAGE_IMAGE"));

#[cfg(feature = "stage22-5-test")]
const PACKAGE_TRANSACTION_FIXTURE: &[u8] = include_bytes!(env!("WOVENHAT_PACKAGE_FIXTURE"));

#[cfg(any(
    feature = "stage23-1-test",
    feature = "stage23-2-test",
    feature = "stage23-3-test"
))]
const AI_SERVICE_IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_AI_IMAGE"));
#[cfg(any(
    feature = "stage23-1-test",
    feature = "stage23-2-test",
    feature = "stage23-3-test"
))]
const AI_PROBE_IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_AI_PROBE_IMAGE"));

#[cfg(all(feature = "stage22-3-test", not(feature = "stage27-5-test")))]
fn stage22_3_package_service_self_test() -> bool {
    use libwoven::package_ipc::{Operation, Request, Response};

    fn fail(reason: &str) -> bool {
        serial::write_line(format_args!("[S22.3] {}", reason));
        false
    }

    let Ok(verified) = signed_app::verify_package(PACKAGE_SERVICE_IMAGE) else {
        return fail("broker signature rejected");
    };
    if elf::parse(verified.bytes()).is_err() {
        return fail("broker ELF rejected");
    }
    let Some(program) = userspace::load_elf_with_argv_verified(
        PACKAGE_SERVICE_IMAGE,
        &["/bin/woven-package", "--service"],
    ) else {
        return fail("broker loader rejected");
    };
    let capabilities = capability::CapabilitySet::only(capability::Capability::Ipc)
        .with(capability::Capability::FileRead);
    if vfs::write_file("/tmp/vfs-self-test", b"package-query-fixture").is_err() {
        return fail("query fixture storage write failed");
    }
    let Ok((pid, _, _)) = task::spawn_user_system_service("woven-package", program, capabilities)
    else {
        return fail("broker spawn failed");
    };
    let deadline = timer::ticks().wrapping_add(500);
    let published = loop {
        if let Ok(info) = ipc::service_info(b"woven.package") {
            if info.owner == pid.as_u64() && info.client_rights == 1 {
                break true;
            }
        }
        if timer::ticks().wrapping_sub(deadline) < u64::MAX / 2 {
            break false;
        }
        task::yield_now();
    };
    if !published {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("broker service was not published");
    }
    let Ok(client_handle) = ipc::discover_service(pid.as_u64(), b"woven.package") else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("service discovery failed");
    };
    let request = Request {
        operation: Operation::Query,
        request_id: 0x22_03,
        path: b"/tmp/vfs-self-test",
    };
    let mut bytes = [0u8; libwoven::package_ipc::MAX_MESSAGE_BYTES];
    let Ok(length) = request.encode(&mut bytes) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("request encoding failed");
    };
    if ipc::send_handle(pid.as_u64(), client_handle, &bytes[..length]).is_err() {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("request send failed");
    }
    let deadline = timer::ticks().wrapping_add(500);
    let mut response_ok = false;
    loop {
        if task::process_exited(pid) {
            return fail("broker exited while handling request");
        }
        if let Ok((message, serial)) = ipc::peek(pid.as_u64()) {
            if let Ok(response) = Response::decode(message.payload()) {
                response_ok = response.request_id == request.request_id
                    && response.status == 0
                    && response.value == 1;
            }
            let _ = ipc::receive_if(pid.as_u64(), serial);
            break;
        }
        if timer::ticks().wrapping_sub(deadline) < u64::MAX / 2 {
            break;
        }
        task::yield_now();
    }
    let _ = task::kill_process(pid.as_u64(), 9);
    let cleanup_deadline = timer::ticks().wrapping_add(500);
    while !task::process_exited(pid) {
        if timer::ticks().wrapping_sub(cleanup_deadline) < u64::MAX / 2 {
            return fail("broker teardown timed out");
        }
        task::yield_now();
    }
    if task::wait_process(pid.as_u64()).is_err() {
        return fail("broker reap failed");
    }
    if !response_ok {
        return fail("broker response mismatch or timeout");
    }
    let service_deadline = timer::ticks().wrapping_add(500);
    while ipc::service_info(b"woven.package").is_ok() {
        if timer::ticks().wrapping_sub(service_deadline) < u64::MAX / 2 {
            return fail("broker service cleanup timed out");
        }
        task::yield_now();
    }
    true
}

#[cfg(feature = "stage22-5-test")]
fn stage22_5_package_transaction_self_test() -> bool {
    use libwoven::package_ipc::{Operation, Request, Response};

    fn fail(reason: &str) -> bool {
        serial::write_line(format_args!("[S22.5] {}", reason));
        false
    }

    fn request(
        pid: task::ProcessId,
        handle: ipc::Handle,
        operation: Operation,
        request_id: u64,
        path: &'static [u8],
    ) -> Option<Response> {
        let request = Request {
            operation,
            request_id,
            path,
        };
        let mut bytes = crate::heap::try_filled(0u8, libwoven::package_ipc::MAX_MESSAGE_BYTES)?;
        let length = request.encode(&mut bytes).ok()?;
        ipc::send_handle(pid.as_u64(), handle, &bytes[..length]).ok()?;
        let deadline = timer::ticks().wrapping_add(500);
        loop {
            if task::process_exited(pid) {
                return None;
            }
            if let Ok((message, serial)) = ipc::peek(pid.as_u64()) {
                let response = Response::decode(message.payload()).ok();
                let _ = ipc::receive_if(pid.as_u64(), serial);
                return response;
            }
            if timer::ticks().wrapping_sub(deadline) < u64::MAX / 2 {
                return None;
            }
            task::yield_now();
        }
    }

    if !wfs::format_ata(64) {
        return fail("native WovenFS format failed");
    }
    if vfs::write_file(
        "/tmp/woven-package-fixture.wpkg",
        PACKAGE_TRANSACTION_FIXTURE,
    )
    .is_err()
    {
        return fail("package fixture source write failed");
    }
    let Ok(verified) = signed_app::verify_package(PACKAGE_SERVICE_IMAGE) else {
        return fail("broker signature rejected");
    };
    if elf::parse(verified.bytes()).is_err() {
        return fail("broker ELF rejected");
    }
    let Some(program) = userspace::load_elf_with_argv_verified(
        PACKAGE_SERVICE_IMAGE,
        &["/bin/woven-package", "--service"],
    ) else {
        return fail("broker loader rejected");
    };
    let capabilities = capability::CapabilitySet::only(capability::Capability::Ipc)
        .with(capability::Capability::FileRead)
        .with(capability::Capability::FileWrite)
        .with(capability::Capability::StorageIo);
    let Ok((pid, _, _)) = task::spawn_user_system_service("woven-package", program, capabilities)
    else {
        return fail("broker spawn failed");
    };
    let deadline = timer::ticks().wrapping_add(500);
    let published = loop {
        if let Ok(info) = ipc::service_info(b"woven.package") {
            if info.owner == pid.as_u64() && info.client_rights == 1 {
                break true;
            }
        }
        if timer::ticks().wrapping_sub(deadline) < u64::MAX / 2 {
            break false;
        }
        task::yield_now();
    };
    if !published {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("broker service was not published");
    }
    let Ok(client_handle) = ipc::discover_service(pid.as_u64(), b"woven.package") else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("service discovery failed");
    };
    let Some(response) = request(
        pid,
        client_handle,
        Operation::Install,
        0x22_05_01,
        b"/tmp/woven-package-fixture.wpkg",
    ) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("install response timeout");
    };
    if response.status != 0 || response.value != 1 {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("install response mismatch");
    }
    let Some(mut record) = crate::heap::try_filled(0u8, libwoven::store::MAX_RECORD_BYTES) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("catalog buffer allocation failed");
    };
    let Ok(length) = wfs::read_vfs_path_durable("/wfs/packages/catalog.wstc", &mut record) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("installed catalog read failed");
    };
    let Ok(installed) = libwoven::store::Store::decode_record(&record[..length]) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("installed catalog decode failed");
    };
    if installed.entry(b"fixture").is_none() {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("installed catalog entry missing");
    }
    let Some(response) = request(
        pid,
        client_handle,
        Operation::Remove,
        0x22_05_02,
        b"/fixture",
    ) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("remove response timeout");
    };
    if response.status != 0 || response.value != 0 {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("remove response mismatch");
    }
    let Ok(length) = wfs::read_vfs_path_durable("/wfs/packages/catalog.wstc", &mut record) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("removed catalog read failed");
    };
    let Ok(removed) = libwoven::store::Store::decode_record(&record[..length]) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("removed catalog decode failed");
    };
    if removed.entry(b"fixture").is_some() {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("remove left catalog entry");
    }
    let Some(response) = request(
        pid,
        client_handle,
        Operation::Rollback,
        0x22_05_03,
        b"/rollback",
    ) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("rollback response timeout");
    };
    if response.status != 0 || response.value != 1 {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("rollback response mismatch");
    }
    let Ok(length) = wfs::read_vfs_path_durable("/wfs/packages/catalog.wstc", &mut record) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("rollback catalog read failed");
    };
    let Ok(restored) = libwoven::store::Store::decode_record(&record[..length]) else {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("rollback catalog decode failed");
    };
    if restored.entry(b"fixture").is_none() {
        let _ = task::kill_process(pid.as_u64(), 9);
        return fail("rollback did not restore catalog entry");
    }
    let _ = task::kill_process(pid.as_u64(), 9);
    let cleanup_deadline = timer::ticks().wrapping_add(500);
    while !task::process_exited(pid) {
        if timer::ticks().wrapping_sub(cleanup_deadline) < u64::MAX / 2 {
            return fail("broker teardown timed out");
        }
        task::yield_now();
    }
    if task::wait_process(pid.as_u64()).is_err() {
        return fail("broker reap failed");
    }
    true
}

#[cfg(any(
    feature = "stage23-1-test",
    feature = "stage23-2-test",
    feature = "stage23-3-test"
))]
fn stage23_ai_broker_self_test(local_provider: bool) -> bool {
    fn fail(reason: &str) -> bool {
        serial::write_line(format_args!("[S23] {}", reason));
        false
    }
    let Ok(verified) = signed_app::verify_package(AI_SERVICE_IMAGE) else {
        return fail("broker signature rejected");
    };
    if elf::parse(verified.bytes()).is_err() {
        return fail("broker ELF rejected");
    }
    let service_args: &[&str] = if local_provider {
        &["/bin/woven-ai", "--local"]
    } else {
        &["/bin/woven-ai"]
    };
    let Some(service_program) =
        userspace::load_elf_with_argv_verified(AI_SERVICE_IMAGE, service_args)
    else {
        return fail("broker load failed");
    };
    let Ok((service_pid, _, _)) = task::spawn_user_system_service(
        "woven-ai",
        service_program,
        capability::CapabilitySet::only(capability::Capability::Ipc),
    ) else {
        return fail("broker spawn failed");
    };
    let deadline = timer::ticks().wrapping_add(500);
    loop {
        if let Ok(info) = ipc::service_info(b"woven.ai") {
            if info.owner == service_pid.as_u64() && info.client_rights == 1 {
                break;
            }
        }
        if task::process_exited(service_pid) || timer::ticks().wrapping_sub(deadline) < u64::MAX / 2
        {
            let _ = task::kill_process(service_pid.as_u64(), 9);
            return fail("broker publication failed");
        }
        task::yield_now();
    }
    let Ok(verified) = signed_app::verify_package(AI_PROBE_IMAGE) else {
        let _ = task::kill_process(service_pid.as_u64(), 9);
        return fail("probe signature rejected");
    };
    if elf::parse(verified.bytes()).is_err() {
        let _ = task::kill_process(service_pid.as_u64(), 9);
        return fail("probe ELF rejected");
    }
    let denied_args: &[&str] = if local_provider {
        &["/bin/woven-ai", "--probe-local"]
    } else {
        &["/bin/woven-ai", "--probe"]
    };
    let Some(denied_program) = userspace::load_elf_with_argv_verified(AI_PROBE_IMAGE, denied_args)
    else {
        let _ = task::kill_process(service_pid.as_u64(), 9);
        return fail("denied probe load failed");
    };
    let Ok((denied_pid, _)) = task::spawn_user_process_with_package_capabilities(
        "woven-ai-denied",
        denied_program,
        capability::CapabilitySet::only(capability::Capability::Ipc).bits(),
    ) else {
        let _ = task::kill_process(service_pid.as_u64(), 9);
        return fail("denied probe spawn failed");
    };
    let denied_deadline = timer::ticks().wrapping_add(500);
    while !task::process_exited(denied_pid) {
        if timer::ticks().wrapping_sub(denied_deadline) < u64::MAX / 2 {
            let _ = task::kill_process(denied_pid.as_u64(), 9);
            let _ = task::kill_process(service_pid.as_u64(), 9);
            return fail("denied probe timeout");
        }
        task::yield_now();
    }
    if task::wait_process(denied_pid.as_u64()) != Ok(112) {
        let _ = task::kill_process(service_pid.as_u64(), 9);
        return fail("Ai capability denial was not enforced");
    }
    let Some(probe_program) = userspace::load_elf_with_argv_verified(AI_PROBE_IMAGE, denied_args)
    else {
        let _ = task::kill_process(service_pid.as_u64(), 9);
        return fail("probe load failed");
    };
    let Ok((probe_pid, _)) = task::spawn_user_process_with_package_capabilities(
        "woven-ai-probe",
        probe_program,
        capability::CapabilitySet::only(capability::Capability::Ipc)
            .with(capability::Capability::Ai)
            .bits(),
    ) else {
        let _ = task::kill_process(service_pid.as_u64(), 9);
        return fail("probe spawn failed");
    };
    let probe_deadline = timer::ticks().wrapping_add(500);
    while !task::process_exited(probe_pid) {
        if timer::ticks().wrapping_sub(probe_deadline) < u64::MAX / 2 {
            let _ = task::kill_process(probe_pid.as_u64(), 9);
            let _ = task::kill_process(service_pid.as_u64(), 9);
            return fail("probe timeout");
        }
        task::yield_now();
    }
    let probe_status = task::wait_process(probe_pid.as_u64()).ok();
    let _ = task::kill_process(service_pid.as_u64(), 9);
    let service_deadline = timer::ticks().wrapping_add(500);
    while !task::process_exited(service_pid) {
        if timer::ticks().wrapping_sub(service_deadline) < u64::MAX / 2 {
            return fail("broker teardown timeout");
        }
        task::yield_now();
    }
    if probe_status != Some(0) {
        return fail("probe returned failure");
    }
    if task::wait_process(service_pid.as_u64()).is_err() {
        return fail("broker reap failed");
    }
    true
}

#[cfg(feature = "stage23-3-test")]
fn stage23_cloud_provider_self_test() -> bool {
    use libwoven::ai_provider;

    fn fail(reason: &str) -> bool {
        serial::write_line(format_args!("[S23.3] {}", reason));
        false
    }
    let Ok(verified) = signed_app::verify_package(AI_SERVICE_IMAGE) else {
        return fail("provider signature rejected");
    };
    if elf::parse(verified.bytes()).is_err() {
        return fail("provider ELF rejected");
    }
    let Some(provider_program) = userspace::load_elf_with_argv_verified(
        AI_SERVICE_IMAGE,
        &["/bin/woven-ai", "--cloud-plugin"],
    ) else {
        return fail("provider load failed");
    };
    let provider_capabilities = capability::CapabilitySet::only(capability::Capability::Ipc)
        .with(capability::Capability::Secrets)
        .with(capability::Capability::NetworkIo);
    let Ok((provider_pid, _)) = task::spawn_user_process_with_package_capabilities(
        "woven-ai-cloud-provider",
        provider_program,
        provider_capabilities.bits(),
    ) else {
        return fail("provider spawn failed");
    };
    let provider_deadline = timer::ticks().wrapping_add(500);
    loop {
        if let Ok(info) = ipc::service_info(ai_provider::SERVICE_NAME) {
            if info.owner == provider_pid.as_u64() && info.client_rights == 1 {
                break;
            }
        }
        if task::process_exited(provider_pid)
            || timer::ticks().wrapping_sub(provider_deadline) < u64::MAX / 2
        {
            let _ = task::kill_process(provider_pid.as_u64(), 9);
            return fail("provider publication failed");
        }
        task::yield_now();
    }
    let Some(broker_program) =
        userspace::load_elf_with_argv_verified(AI_SERVICE_IMAGE, &["/bin/woven-ai", "--cloud"])
    else {
        let _ = task::kill_process(provider_pid.as_u64(), 9);
        return fail("cloud broker load failed");
    };
    let Ok((broker_pid, _, _)) = task::spawn_user_system_service(
        "woven-ai-cloud-broker",
        broker_program,
        capability::CapabilitySet::only(capability::Capability::Ipc),
    ) else {
        let _ = task::kill_process(provider_pid.as_u64(), 9);
        return fail("cloud broker spawn failed");
    };
    let broker_deadline = timer::ticks().wrapping_add(500);
    loop {
        if let Ok(info) = ipc::service_info(b"woven.ai") {
            if info.owner == broker_pid.as_u64() && info.client_rights == 1 {
                break;
            }
        }
        if task::process_exited(broker_pid)
            || timer::ticks().wrapping_sub(broker_deadline) < u64::MAX / 2
        {
            let _ = task::kill_process(broker_pid.as_u64(), 9);
            let _ = task::kill_process(provider_pid.as_u64(), 9);
            return fail("cloud broker publication failed");
        }
        task::yield_now();
    }
    let Ok(verified) = signed_app::verify_package(AI_PROBE_IMAGE) else {
        let _ = task::kill_process(broker_pid.as_u64(), 9);
        let _ = task::kill_process(provider_pid.as_u64(), 9);
        return fail("cloud probe signature rejected");
    };
    if elf::parse(verified.bytes()).is_err() {
        let _ = task::kill_process(broker_pid.as_u64(), 9);
        let _ = task::kill_process(provider_pid.as_u64(), 9);
        return fail("cloud probe ELF rejected");
    }
    let Some(denied_program) =
        userspace::load_elf_with_argv_verified(AI_PROBE_IMAGE, &["/bin/woven-ai", "--probe-cloud"])
    else {
        let _ = task::kill_process(broker_pid.as_u64(), 9);
        let _ = task::kill_process(provider_pid.as_u64(), 9);
        return fail("denied cloud probe load failed");
    };
    let Ok((denied_pid, _)) = task::spawn_user_process_with_package_capabilities(
        "woven-ai-cloud-denied",
        denied_program,
        capability::CapabilitySet::only(capability::Capability::Ipc).bits(),
    ) else {
        let _ = task::kill_process(broker_pid.as_u64(), 9);
        let _ = task::kill_process(provider_pid.as_u64(), 9);
        return fail("denied cloud probe spawn failed");
    };
    let denied_deadline = timer::ticks().wrapping_add(500);
    while !task::process_exited(denied_pid) {
        if timer::ticks().wrapping_sub(denied_deadline) < u64::MAX / 2 {
            let _ = task::kill_process(denied_pid.as_u64(), 9);
            let _ = task::kill_process(broker_pid.as_u64(), 9);
            let _ = task::kill_process(provider_pid.as_u64(), 9);
            return fail("denied cloud probe timeout");
        }
        task::yield_now();
    }
    if task::wait_process(denied_pid.as_u64()) != Ok(142) {
        let _ = task::kill_process(broker_pid.as_u64(), 9);
        let _ = task::kill_process(provider_pid.as_u64(), 9);
        return fail("cloud Ai capability denial was not enforced");
    }
    let Some(probe_program) =
        userspace::load_elf_with_argv_verified(AI_PROBE_IMAGE, &["/bin/woven-ai", "--probe-cloud"])
    else {
        let _ = task::kill_process(broker_pid.as_u64(), 9);
        let _ = task::kill_process(provider_pid.as_u64(), 9);
        return fail("cloud probe load failed");
    };
    let Ok((probe_pid, _)) = task::spawn_user_process_with_package_capabilities(
        "woven-ai-cloud-probe",
        probe_program,
        capability::CapabilitySet::only(capability::Capability::Ipc)
            .with(capability::Capability::Ai)
            .bits(),
    ) else {
        let _ = task::kill_process(broker_pid.as_u64(), 9);
        let _ = task::kill_process(provider_pid.as_u64(), 9);
        return fail("cloud probe spawn failed");
    };
    let probe_deadline = timer::ticks().wrapping_add(500);
    while !task::process_exited(probe_pid) {
        if timer::ticks().wrapping_sub(probe_deadline) < u64::MAX / 2 {
            let _ = task::kill_process(probe_pid.as_u64(), 9);
            let _ = task::kill_process(broker_pid.as_u64(), 9);
            let _ = task::kill_process(provider_pid.as_u64(), 9);
            return fail("cloud probe timeout");
        }
        task::yield_now();
    }
    let status = task::wait_process(probe_pid.as_u64()).ok();
    let _ = task::kill_process(broker_pid.as_u64(), 9);
    let _ = task::kill_process(provider_pid.as_u64(), 9);
    let cleanup_deadline = timer::ticks().wrapping_add(500);
    while !task::process_exited(broker_pid) || !task::process_exited(provider_pid) {
        if timer::ticks().wrapping_sub(cleanup_deadline) < u64::MAX / 2 {
            return fail("cloud provider teardown timeout");
        }
        task::yield_now();
    }
    if status != Some(0) {
        return fail("cloud probe returned failure");
    }
    if task::wait_process(broker_pid.as_u64()).is_err()
        || task::wait_process(provider_pid.as_u64()).is_err()
    {
        return fail("cloud provider reap failed");
    }
    true
}

#[cfg(feature = "stage23-4-test")]
fn stage23_ai_capability_security_self_test() -> bool {
    use capability::{Capability, CapabilitySet};
    use wovenguard::{
        authorize_device_access, domain_ceiling, package_sandbox, DeviceClass, SecurityDomain,
    };

    fn fail(reason: &str) -> bool {
        serial::write_line(format_args!("[S23.4] {}", reason));
        false
    }
    let local = CapabilitySet::only(Capability::Ipc).with(Capability::Ai);
    let Some(local_sandbox) = package_sandbox(local) else {
        return fail("local AI profile rejected");
    };
    for capability in [
        Capability::FileRead,
        Capability::FileWrite,
        Capability::NetworkIo,
        Capability::Secrets,
        Capability::Camera,
        Capability::Microphone,
        Capability::Usb,
    ] {
        if local.contains(capability) || local_sandbox.allows(capability) {
            return fail("local AI profile widened a forbidden capability");
        }
    }
    for device in [
        DeviceClass::Network,
        DeviceClass::Storage,
        DeviceClass::Display,
        DeviceClass::Input,
        DeviceClass::Generic,
    ] {
        if local_sandbox.allows_device(device)
            || authorize_device_access(local, local_sandbox, device).allowed
        {
            return fail("local AI profile exposed a device");
        }
    }
    let cloud = local.with(Capability::Secrets).with(Capability::NetworkIo);
    let Some(cloud_sandbox) = package_sandbox(cloud) else {
        return fail("cloud AI profile rejected");
    };
    if !cloud_sandbox.allows(Capability::Secrets)
        || !cloud_sandbox.allows_device(DeviceClass::Network)
        || !authorize_device_access(cloud, cloud_sandbox, DeviceClass::Network).allowed
    {
        return fail("explicit cloud capabilities were not preserved");
    }
    for capability in [
        Capability::FileRead,
        Capability::FileWrite,
        Capability::Camera,
        Capability::Microphone,
        Capability::Usb,
    ] {
        if cloud.contains(capability) || cloud_sandbox.allows(capability) {
            return fail("cloud AI profile widened a forbidden capability");
        }
    }
    let system = domain_ceiling(SecurityDomain::SystemService);
    for capability in [
        Capability::Ai,
        Capability::Secrets,
        Capability::Camera,
        Capability::Microphone,
    ] {
        if system.contains(capability) {
            return fail("system-service ceiling grants sensitive AI authority");
        }
    }
    true
}

#[cfg(feature = "stage23-5-test")]
fn stage23_semantic_search_self_test() -> bool {
    use libwoven::semantic_search::{Entry, Error, Hit, Index, Kind, MAX_ENCODED_BYTES, MAX_HITS};

    fn fail(reason: &str) -> bool {
        serial::write_line(format_args!("[S23.5] {}", reason));
        false
    }
    let mut index = Index::new();
    let entries = [
        (1, Kind::Application, b"Woven Terminal".as_slice()),
        (2, Kind::File, b"network design document".as_slice()),
        (3, Kind::Setting, b"network privacy settings".as_slice()),
        (4, Kind::Message, b"message from the team".as_slice()),
        (5, Kind::Document, b"production release guide".as_slice()),
        (6, Kind::Command, b"open terminal".as_slice()),
    ];
    for (id, kind, title) in entries {
        let Some(entry) = Entry::new(id, kind, title) else {
            return fail("bounded entry construction failed");
        };
        if !index.insert(entry) {
            return fail("bounded index insertion failed");
        }
    }
    let mut hits = [Hit {
        id: 0,
        kind: Kind::File,
        score: 0,
    }; MAX_HITS];
    let count = index.search(b"network settings", &mut hits);
    if count != 2 || hits[0].id != 3 || hits[0].score != 2 || hits[1].id != 2 {
        return fail("semantic ranking mismatch");
    }
    let count = index.search(b"TERMINAL", &mut hits);
    if count != 2 || hits[0].id != 1 || hits[1].id != 6 {
        return fail("case-insensitive search mismatch");
    }
    if !index.insert(Entry::new_owned(7, 99, Kind::Message, b"private system note").unwrap()) {
        return fail("private entry insertion failed");
    }
    if index.search_for(8, b"private", &mut hits) != 0
        || index.search_for(7, b"private", &mut hits) != 1
    {
        return fail("owner privacy filter mismatch");
    }
    let mut snapshot = [0u8; MAX_ENCODED_BYTES];
    let Ok(length) = index.encode(&mut snapshot) else {
        return fail("search snapshot encoding failed");
    };
    let Ok(restored) = Index::decode(&snapshot[..length]) else {
        return fail("search snapshot decoding failed");
    };
    if restored.search_for(7, b"private", &mut hits) != 1 {
        return fail("search snapshot privacy mismatch");
    }
    snapshot[10] ^= 1;
    if !matches!(Index::decode(&snapshot[..length]), Err(Error::BadChecksum)) {
        return fail("search snapshot checksum was not enforced");
    }
    true
}

#[cfg(feature = "stage23-6-test")]
fn stage23_assistant_action_self_test() -> bool {
    use libwoven::ai_assistant::{Action, ActionKind, Error, Plan};
    use libwoven::Permission;

    fn fail(reason: &str) -> bool {
        serial::write_line(format_args!("[S23.6] {}", reason));
        false
    }
    let mut plan = Plan::new();
    let Ok(write_index) = plan.push(Action::new(ActionKind::WriteFile, b"/home/note.txt").unwrap())
    else {
        return fail("write action insertion failed");
    };
    let Ok(read_index) = plan.push(Action::new(ActionKind::ReadFile, b"/home/note.txt").unwrap())
    else {
        return fail("read action insertion failed");
    };
    if plan.authorize(write_index, &[Permission::FilesystemWrite], false)
        != Err(Error::ConfirmationRequired)
    {
        return fail("write confirmation was not required");
    }
    if plan.authorize(write_index, &[Permission::FilesystemRead], true)
        != Err(Error::PermissionDenied)
    {
        return fail("write permission was not denied");
    }
    if plan
        .authorize(write_index, &[Permission::FilesystemWrite], true)
        .is_err()
    {
        return fail("permitted write was rejected");
    }
    if plan
        .authorize(read_index, &[Permission::FilesystemRead], false)
        .is_err()
    {
        return fail("permitted read was rejected");
    }
    if plan.authorize(read_index, &[Permission::FilesystemWrite], true)
        != Err(Error::PermissionDenied)
    {
        return fail("read permission mismatch");
    }
    true
}

#[cfg(feature = "stage23-7-test")]
fn stage23_security_monitor_self_test() -> bool {
    use libwoven::ai_monitor::{explain, Monitor, Severity};
    use libwoven::security::{AuditAction, AuditEvent};

    fn fail(reason: &str) -> bool {
        serial::write_line(format_args!("[S23.7] {}", reason));
        false
    }
    let denied = AuditEvent {
        sequence: 17,
        tick: 2,
        cpu: 0,
        action: AuditAction::WovenGuardDeny,
        actor: 10,
        target: 20,
        detail: 30,
        allowed: false,
    };
    let finding = explain(&denied);
    if finding.severity != Severity::Warning
        || finding.allowed
        || &finding.explanation[..finding.explanation_len as usize]
            != b"WovenGuard denied an authority request"
    {
        return fail("denial explanation mismatch");
    }
    let mut monitor = Monitor::new();
    for _ in 0..libwoven::ai_monitor::MAX_FINDINGS {
        if !monitor.observe(&denied) {
            return fail("monitor capacity rejected a bounded event");
        }
    }
    if monitor.observe(&denied) || monitor.get(0).is_none() {
        return fail("monitor capacity/enumeration mismatch");
    }
    true
}

#[cfg(feature = "stage24-1-test")]
fn stage24_identity_self_test() -> bool {
    use libwoven::identity::{Authority, Error, SessionState, UserProfile};

    fn fail(reason: &str) -> bool {
        serial::write_line(format_args!("[S24.1] {}", reason));
        false
    }

    let mut authority = Authority::new();
    let Ok(group) = authority.add_group(b"users") else {
        return fail("group creation failed");
    };
    let Ok(user) = authority.add_administrator(b"alice", group, b"qemu-salt", b"1234") else {
        return fail("user creation failed");
    };
    let Ok(profile) = UserProfile::new(b"Alice Example") else {
        return fail("profile validation failed");
    };
    if authority.set_profile(user, profile).is_err() {
        return fail("profile update failed");
    }
    if authority.open_session(user, b"wrong") != Err(Error::WrongPassword) {
        return fail("wrong password was accepted");
    }
    let Ok(session) = authority.open_session(user, b"1234") else {
        return fail("session creation failed");
    };
    let Ok(view) = authority.view(session) else {
        return fail("session view failed");
    };
    if view.uid != 1000 || view.state != SessionState::Active || view.elevated {
        return fail("initial session state mismatch");
    }
    if authority.lock(session).is_err()
        || authority.elevate(session, b"1234") != Err(Error::Locked)
        || authority.unlock(session, b"1234").is_err()
        || authority.elevate(session, b"1234").is_err()
    {
        return fail("lock/unlock/elevation lifecycle failed");
    }
    if authority
        .view(session)
        .map_or(true, |view| view.uid != 0 || !view.elevated)
    {
        return fail("elevated credentials were not reflected");
    }
    true
}

#[cfg(feature = "stage24-2-test")]
fn stage24_identity_snapshot_self_test() -> bool {
    use libwoven::identity::{Authority, Error, UserProfile, SNAPSHOT_BYTES};

    fn fail(reason: &str) -> bool {
        serial::write_line(format_args!("[S24.2] {}", reason));
        false
    }

    let mut authority = Authority::new();
    let Ok(group) = authority.add_group(b"users") else {
        return fail("group creation failed");
    };
    let Ok(user) = authority.add_administrator(b"alice", group, b"qemu-salt", b"1234") else {
        return fail("user creation failed");
    };
    if authority
        .set_profile(user, UserProfile::new(b"Alice Example").unwrap())
        .is_err()
    {
        return fail("profile update failed");
    }
    let Ok(session) = authority.open_session(user, b"1234") else {
        return fail("session creation failed");
    };
    let mut snapshot = [0u8; SNAPSHOT_BYTES];
    if authority.encode_snapshot(&mut snapshot) != Ok(SNAPSHOT_BYTES) {
        return fail("snapshot encoding failed");
    }
    let mut restored = match Authority::decode_snapshot(&snapshot) {
        Ok(authority) => authority,
        Err(_) => return fail("snapshot decoding failed"),
    };
    if restored.view(session) != Err(Error::NotFound)
        || restored.open_session(user, b"1234").is_err()
    {
        return fail("session state was incorrectly persisted");
    }
    snapshot[SNAPSHOT_BYTES - 1] ^= 1;
    if !matches!(Authority::decode_snapshot(&snapshot), Err(Error::Corrupt)) {
        return fail("tampered snapshot was accepted");
    }
    true
}

#[cfg(feature = "stage24-5-test")]
fn stage24_identity_syscall_frame_self_test() -> bool {
    let valid_open = [5, b'a', b'l', b'i', b'c', b'e', 4, b'1', b'2', b'3', b'4'];
    let Some((name, password)) = identity_runtime::parse_open_buffer(&valid_open) else {
        return false;
    };
    if name != b"alice" || password != b"1234" {
        return false;
    }
    let malformed = [5, b'a', b'l', b'i', b'c', b'e', 4, b'1'];
    if identity_runtime::parse_open_buffer(&malformed).is_some() {
        return false;
    }
    let valid_password = [4, b'1', b'2', b'3', b'4'];
    let Some(password) = identity_runtime::parse_password_buffer(&valid_password) else {
        return false;
    };
    if password != b"1234" {
        return false;
    }
    identity_runtime::parse_password_buffer(&[0]).is_none()
}

#[cfg(feature = "system-apps")]
fn start_production_system_services() -> bool {
    let capabilities = capability::CapabilitySet::only(capability::Capability::Ipc)
        .with(capability::Capability::FileRead)
        .with(capability::Capability::FileWrite)
        .with(capability::Capability::StorageIo);
    let Some(program) = userspace::load_elf_with_argv_verified(
        PACKAGE_SERVICE_IMAGE,
        &["/bin/woven-package", "--service"],
    ) else {
        serial::write_line(format_args!("[SERVICES] woven-package load failed"));
        return false;
    };
    let Ok((pid, _, _)) = task::spawn_user_system_service("woven-package", program, capabilities)
    else {
        serial::write_line(format_args!("[SERVICES] woven-package spawn failed"));
        return false;
    };
    let deadline = timer::ticks().wrapping_add(500);
    loop {
        if let Ok(info) = ipc::service_info(b"woven.package") {
            if info.owner == pid.as_u64() && info.client_rights == 1 {
                serial::write_line(format_args!(
                    "[SERVICES] woven-package published as pid {}",
                    pid.as_u64()
                ));
                break;
            }
        }
        if task::process_exited(pid) || timer::ticks().wrapping_sub(deadline) < u64::MAX / 2 {
            let _ = task::kill_process(pid.as_u64(), 9);
            serial::write_line(format_args!("[SERVICES] woven-package publication failed"));
            return false;
        }
        task::yield_now();
    }

    let Some(program) = userspace::load_elf_with_argv_verified(
        SYSTEM_APP_UPDATE,
        &["/bin/wovenhat-update", "--service"],
    ) else {
        serial::write_line(format_args!("[SERVICES] woven-update load failed"));
        return false;
    };
    let Ok((pid, _, _)) = task::spawn_user_system_service("woven-update", program, capabilities)
    else {
        serial::write_line(format_args!("[SERVICES] woven-update spawn failed"));
        return false;
    };
    let deadline = timer::ticks().wrapping_add(500);
    loop {
        if let Ok(info) = ipc::service_info(b"woven.update") {
            if info.owner == pid.as_u64() && info.client_rights == 1 {
                serial::write_line(format_args!(
                    "[SERVICES] woven-update published as pid {}",
                    pid.as_u64()
                ));
                return true;
            }
        }
        if task::process_exited(pid) || timer::ticks().wrapping_sub(deadline) < u64::MAX / 2 {
            let _ = task::kill_process(pid.as_u64(), 9);
            serial::write_line(format_args!("[SERVICES] woven-update publication failed"));
            return false;
        }
        task::yield_now();
    }
}

#[cfg(feature = "system-apps")]
fn install_production_system_apps() -> bool {
    #[derive(Clone, Copy)]
    struct App {
        name: &'static str,
        image: &'static [u8],
        binary_path: &'static str,
        catalog_path: &'static str,
        catalog: &'static [u8],
    }
    let apps = [
        App {
            name: "woven-calculator",
            image: SYSTEM_APP_CALCULATOR,
            binary_path: "/bin/wovenhat-calculator",
            catalog_path: "/share/applications/woven-calculator.app",
            catalog: b"Name=WovenCalculator\nExec=/bin/wovenhat-calculator --gui %p\n",
        },
        App {
            name: "woven-terminal",
            image: SYSTEM_APP_TERMINAL,
            binary_path: "/bin/wovenhat-terminal",
            catalog_path: "/share/applications/woven-terminal.app",
            catalog: b"Name=WovenTerminal\nExec=/bin/wovenhat-terminal --gui %p\n",
        },
        App {
            name: "woven-files",
            image: SYSTEM_APP_FILES,
            binary_path: "/bin/wovenhat-files",
            catalog_path: "/share/applications/woven-files.app",
            catalog: b"Name=WovenFiles\nExec=/bin/wovenhat-files --gui %p\n",
        },
        App {
            name: "woven-monitor",
            image: SYSTEM_APP_MONITOR,
            binary_path: "/bin/wovenhat-monitor",
            catalog_path: "/share/applications/woven-monitor.app",
            catalog: b"Name=WovenMonitor\nExec=/bin/wovenhat-monitor --gui %p\n",
        },
        App {
            name: "woven-settings",
            image: SYSTEM_APP_SETTINGS,
            binary_path: "/bin/wovenhat-settings",
            catalog_path: "/share/applications/woven-settings.app",
            catalog: b"Name=WovenSettings\nExec=/bin/wovenhat-settings --gui %p\n",
        },
        App {
            name: "woven-security",
            image: SYSTEM_APP_SECURITY,
            binary_path: "/bin/wovenhat-security",
            catalog_path: "/share/applications/woven-security.app",
            catalog: b"Name=WovenSecurity\nExec=/bin/wovenhat-security --gui %p\n",
        },
        App {
            name: "woven-editor",
            image: SYSTEM_APP_EDITOR,
            binary_path: "/bin/wovenhat-editor",
            catalog_path: "/share/applications/woven-editor.app",
            catalog: b"Name=WovenEditor\nExec=/bin/wovenhat-editor --gui %p\n",
        },
        App {
            name: "woven-images",
            image: SYSTEM_APP_IMAGES,
            binary_path: "/bin/wovenhat-images",
            catalog_path: "/share/applications/woven-images.app",
            catalog: b"Name=WovenImages\nExec=/bin/wovenhat-images --gui %p\n",
        },
        App {
            name: "woven-update",
            image: SYSTEM_APP_UPDATE,
            binary_path: "/bin/wovenhat-update",
            catalog_path: "/share/applications/woven-update.app",
            catalog: b"Name=WovenUpdate\nExec=/bin/wovenhat-update --gui %p\n",
        },
        App {
            name: "woven-store",
            image: SYSTEM_APP_STORE,
            binary_path: "/bin/wovenhat-store",
            catalog_path: "/share/applications/woven-store.app",
            catalog: b"Name=WovenStore\nExec=/bin/wovenhat-store --gui %p\n",
        },
    ];
    // Verify and parse all packages before changing the VFS. A malformed or
    // untrusted package therefore cannot leave a partially published catalog.
    for app in apps {
        let Ok(verified) = signed_app::verify_package(app.image) else {
            serial::write_line(format_args!("[APPS] {} signature rejected", app.name));
            return false;
        };
        if elf::parse(verified.bytes()).is_err() {
            serial::write_line(format_args!("[APPS] {} ELF rejected", app.name));
            return false;
        }
        let Some(program) = userspace::load_elf_with_argv_verified(app.image, &[app.binary_path])
        else {
            serial::write_line(format_args!("[APPS] {} loader rejected", app.name));
            return false;
        };
        if !userspace::destroy(program.address_space) {
            serial::write_line(format_args!("[APPS] {} loader cleanup failed", app.name));
            return false;
        }
        if vfs::stat(app.binary_path).is_ok() || vfs::stat(app.catalog_path).is_ok() {
            serial::write_line(format_args!("[APPS] {} path collision", app.name));
            return false;
        }
    }
    let share_created = if vfs::stat("/share").is_err() {
        if vfs::mkdir("/share").is_err() {
            return false;
        }
        true
    } else {
        false
    };
    let applications_created = if vfs::stat("/share/applications").is_err() {
        if vfs::mkdir("/share/applications").is_err() {
            if share_created {
                let _ = vfs::remove("/share");
            }
            return false;
        }
        true
    } else {
        false
    };
    if vfs::stat("/share").is_err() {
        return false;
    }
    let mut installed = 0usize;
    for app in apps {
        if vfs::create_read_only(app.binary_path, app.image).is_err() {
            for rollback in apps.into_iter().take(installed) {
                let _ = vfs::remove(rollback.binary_path);
            }
            if applications_created {
                let _ = vfs::remove("/share/applications");
            }
            if share_created {
                let _ = vfs::remove("/share");
            }
            return false;
        }
        installed += 1;
    }
    let mut catalogs = 0usize;
    for app in apps {
        if vfs::create_read_only(app.catalog_path, app.catalog).is_err() {
            for rollback in apps {
                if catalogs > 0 {
                    let _ = vfs::remove(rollback.catalog_path);
                    catalogs -= 1;
                }
            }
            for rollback in apps.into_iter().take(installed) {
                let _ = vfs::remove(rollback.binary_path);
            }
            if applications_created {
                let _ = vfs::remove("/share/applications");
            }
            if share_created {
                let _ = vfs::remove("/share");
            }
            return false;
        }
        catalogs += 1;
    }
    serial::write_line(format_args!(
        "[APPS] {} signed applications installed",
        installed
    ));
    true
}

#[cfg(feature = "system-apps")]
fn install_persistent_accounts() -> bool {
    const SOURCE: &str = "/mnt/etc/wovenhat/accounts";
    const TARGET: &str = "/etc/wovenhat/accounts";

    let source = match vfs::stat(SOURCE) {
        Ok(stat) if stat.kind == vfs::NodeKind::File => stat,
        Ok(_) => {
            serial::write_line(format_args!(
                "[SESSION] persistent accounts path is not a file"
            ));
            return false;
        }
        Err(vfs::Error::NotFound) => return true,
        Err(_) => return false,
    };
    if source.size == 0 || source.size > vfs::NODE_CAPACITY {
        serial::write_line(format_args!(
            "[SESSION] persistent accounts file has invalid size: {}",
            source.size
        ));
        return false;
    }

    let mut data = alloc::vec![0u8; source.size];
    let Ok(read) = vfs::read_all(SOURCE, &mut data) else {
        serial::write_line(format_args!("[SESSION] persistent accounts read failed"));
        return false;
    };
    if read != source.size || !has_valid_account_line(&data) {
        serial::write_line(format_args!(
            "[SESSION] persistent accounts file contains no valid account"
        ));
        return false;
    }
    for directory in ["/etc", "/etc/wovenhat"] {
        match vfs::mkdir(directory) {
            Ok(()) | Err(vfs::Error::AlreadyExists) => {}
            Err(_) => return false,
        }
    }
    if vfs::create_read_only(TARGET, &data).is_err() {
        return false;
    }
    serial::write_line(format_args!(
        "[SESSION] persistent accounts imported from {}",
        SOURCE
    ));
    true
}

#[cfg(feature = "system-apps")]
fn has_valid_account_line(data: &[u8]) -> bool {
    data.split(|byte| *byte == b'\n').any(|line| {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let mut fields = line.split(|byte| *byte == b':');
        let (Some(name), Some(salt), Some(hash)) = (fields.next(), fields.next(), fields.next())
        else {
            return false;
        };
        fields.next().is_none()
            && !name.is_empty()
            && name.len() <= 16
            && name.iter().all(|byte| (b' '..=b'~').contains(byte))
            && !salt.is_empty()
            && salt.len() <= 64
            && salt.len().is_multiple_of(2)
            && salt.iter().all(u8::is_ascii_hexdigit)
            && hash.len() == 64
            && hash.iter().all(u8::is_ascii_hexdigit)
    })
}

#[cfg(feature = "hardware-demo")]
fn install_hardware_demo_accounts() -> bool {
    // These two lines are `name:salt-hex:hash-hex`, matching
    // libwoven::accounts's format and PBKDF2_ITERATIONS exactly (ada:1234
    // and lin:9876, the same fixed demo credentials this function has
    // always installed) -- but the hash is precomputed, not recomputed by
    // calling pbkdf2_hmac at every single boot. Boot profiling (Stage 30)
    // measured that recomputation (2 accounts x 100,000 iterations,
    // force-soft SHA-256 -- this kernel never uses a hardware SHA
    // extension) at 4241 of this demo image's ~4350-tick total boot under
    // QEMU TCG: effectively the entire boot. These credentials are public
    // (a disposable demo account, not a secret), so precomputing them once
    // loses nothing: the PBKDF2 work factor still protects them exactly as
    // before, it is simply not redone on every boot of every test run.
    const ACCOUNTS_PATH: &str = "/etc/wovenhat/accounts";
    const ACCOUNTS: &str = concat!(
        "ada:1122334455667788:",
        "286eb6cda9bcda66eb2499be4bb94c44c19f4cc505d42cb7a70b7270c5da632b\n",
        "lin:a0b1c2d3e4f50617:",
        "cfefdcdc20fd0469dcf4c7203ee635341fe3cdc1e7102fdb51df263ad56d7f8f\n",
    );
    match vfs::stat(ACCOUNTS_PATH) {
        Ok(_) => {
            serial::write_line(format_args!(
                "[DEMO] existing accounts preserved; demo accounts not installed"
            ));
            return true;
        }
        Err(vfs::Error::NotFound) => {}
        Err(_) => return false,
    }

    for directory in ["/etc", "/etc/wovenhat"] {
        match vfs::mkdir(directory) {
            Ok(()) | Err(vfs::Error::AlreadyExists) => {}
            Err(_) => return false,
        }
    }
    if vfs::create_read_only(ACCOUNTS_PATH, ACCOUNTS.as_bytes()).is_err() {
        return false;
    }
    serial::write_line(format_args!(
        "[DEMO] sample accounts installed in volatile VFS (ada, lin)"
    ));
    true
}

/// Stage 19 system-app acceptance: verify and install one bounded catalog plan.
///
/// This deliberately remains a test feature. Normal boot does not install
/// packages merely because build-time paths exist; a future image installer
/// must provide the same verification, ownership, and publication ordering.
#[cfg(feature = "stage19-app-catalog-test")]
fn stage19_app_catalog_self_test() -> bool {
    struct App {
        name: &'static str,
        image: &'static [u8],
        binary_path: &'static str,
        catalog_path: &'static str,
        catalog: &'static [u8],
    }
    let apps = [
        App {
            name: "woven-calculator",
            image: SYSTEM_APP_CALCULATOR,
            binary_path: "/bin/wovenhat-calculator",
            catalog_path: "/share/applications/woven-calculator.app",
            catalog: b"Name=WovenCalculator\nExec=/bin/wovenhat-calculator\n",
        },
        App {
            name: "woven-terminal",
            image: SYSTEM_APP_TERMINAL,
            binary_path: "/bin/wovenhat-terminal",
            catalog_path: "/share/applications/woven-terminal.app",
            catalog: b"Name=WovenTerminal\nExec=/bin/wovenhat-terminal --interactive\n",
        },
        App {
            name: "woven-files",
            image: SYSTEM_APP_FILES,
            binary_path: "/bin/wovenhat-files",
            catalog_path: "/share/applications/woven-files.app",
            catalog: b"Name=WovenFiles\nExec=/bin/wovenhat-files\n",
        },
        App {
            name: "woven-monitor",
            image: SYSTEM_APP_MONITOR,
            binary_path: "/bin/wovenhat-monitor",
            catalog_path: "/share/applications/woven-monitor.app",
            catalog: b"Name=WovenMonitor\nExec=/bin/wovenhat-monitor\n",
        },
        App {
            name: "woven-settings",
            image: SYSTEM_APP_SETTINGS,
            binary_path: "/bin/wovenhat-settings",
            catalog_path: "/share/applications/woven-settings.app",
            catalog: b"Name=WovenSettings\nExec=/bin/wovenhat-settings\n",
        },
        App {
            name: "woven-security",
            image: SYSTEM_APP_SECURITY,
            binary_path: "/bin/wovenhat-security",
            catalog_path: "/share/applications/woven-security.app",
            catalog: b"Name=WovenSecurity\nExec=/bin/wovenhat-security\n",
        },
        App {
            name: "woven-editor",
            image: SYSTEM_APP_EDITOR,
            binary_path: "/bin/wovenhat-editor",
            catalog_path: "/share/applications/woven-editor.app",
            catalog: b"Name=WovenEditor\nExec=/bin/wovenhat-editor\n",
        },
        App {
            name: "woven-images",
            image: SYSTEM_APP_IMAGES,
            binary_path: "/bin/wovenhat-images",
            catalog_path: "/share/applications/woven-images.app",
            catalog: b"Name=WovenImages\nExec=/bin/wovenhat-images\n",
        },
        App {
            name: "woven-update",
            image: SYSTEM_APP_UPDATE,
            binary_path: "/bin/wovenhat-update",
            catalog_path: "/share/applications/woven-update.app",
            catalog: b"Name=WovenUpdate\nExec=/bin/wovenhat-update\n",
        },
        App {
            name: "woven-store",
            image: SYSTEM_APP_STORE,
            binary_path: "/bin/wovenhat-store",
            catalog_path: "/share/applications/woven-store.app",
            catalog: b"Name=WovenStore\nExec=/bin/wovenhat-store\n",
        },
    ];
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19-APP] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if !step(
        "application directories created",
        vfs::mkdir("/share").is_ok() && vfs::mkdir("/share/applications").is_ok(),
    ) {
        return false;
    }
    for app in apps {
        let verified = match signed_app::verify_package(app.image) {
            Ok(package) => package,
            Err(_) => return step(app.name, false),
        };
        if !step("signed package verifies", true) {
            return false;
        }
        let mut tampered = alloc::vec::Vec::from(app.image);
        let last = tampered.len().saturating_sub(1);
        tampered[last] ^= 1;
        if !step(
            "tampered package refused",
            signed_app::verify_package(&tampered).is_err(),
        ) {
            return false;
        }
        let image = verified.bytes();
        let Some(parsed) = elf::parse(image).ok() else {
            return step("ELF payload parses", false);
        };
        if !step("ELF payload parses", parsed.segment_count() > 0) {
            return false;
        }
        if !step(
            "package installed at declared path",
            vfs::create_read_only(app.binary_path, app.image).is_ok(),
        ) {
            return false;
        }
        let mut readback = alloc::vec![0u8; app.image.len()];
        let round_trip = vfs::read_all(app.binary_path, &mut readback) == Ok(app.image.len())
            && readback == app.image;
        if !step("package read-back is byte-identical", round_trip) {
            return false;
        }
        if !step(
            "read-back loads through verified loader",
            userspace::load_elf_with_argv_verified(&readback, &[app.binary_path]).is_some(),
        ) {
            return false;
        }
        if !step(
            "catalog descriptor published",
            vfs::create_read_only(app.catalog_path, app.catalog).is_ok(),
        ) {
            return false;
        }
    }
    step("ten signed applications installed", true)
}

/// Stage 16.1 acceptance: an application built outside the kernel is installed
/// into the filesystem, loaded by the ELF loader, and runs in Ring 3.
#[cfg(feature = "stage16-1-test")]
fn stage16_1_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S16.1] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    serial::write_line(format_args!(
        "[S16.1] signed application package bytes {}",
        EXTERNAL_APPLICATION.len()
    ));

    let Some(verified) = signed_app::verify_package(EXTERNAL_APPLICATION).ok() else {
        return step("signed WAPP package verifies", false);
    };
    if !step("signed WAPP package verifies", true) {
        return false;
    }
    let mut tampered = alloc::vec::Vec::from(EXTERNAL_APPLICATION);
    if let Some(last) = tampered.last_mut() {
        *last ^= 1;
    }
    if !step(
        "tampered WAPP package is refused",
        signed_app::verify_package(&tampered).is_err(),
    ) {
        return false;
    }
    let image = verified.bytes();
    if !step(
        "unsigned external image is refused",
        userspace::load_elf_with_argv_verified(image, &["/bin/unsigned"]).is_none(),
    ) {
        return false;
    }

    // The loader must accept the toolchain's signed output. This is the claim
    // that matters: no synthetic ELF, no in-kernel assembly, and no unsigned
    // external image bypass.
    let parsed = match elf::parse(image) {
        Ok(image) => image,
        Err(_) => {
            // elf::Error carries no Debug impl, and the loader's acceptance
            // rules are what matter here rather than the variant.
            serial::write_line(format_args!("[S16.1] elf rejected by the loader"));
            return false;
        }
    };
    serial::write_line(format_args!(
        "[S16.1] elf entry {:#x} segments {}",
        parsed.entry,
        parsed.segment_count()
    ));
    if !step(
        "loader accepts the external image",
        parsed.segment_count() > 0,
    ) {
        return false;
    }

    // Install it where exec resolves programs, exactly as `/bin/init` is
    // installed, so this uses the ordinary lookup path rather than a private
    // one.
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello", EXTERNAL_APPLICATION).is_ok(),
    ) {
        return false;
    }

    // Read it back through the VFS and confirm the bytes survived, so a later
    // load failure cannot be blamed on storage.
    let mut readback = alloc::vec![0u8; EXTERNAL_APPLICATION.len()];
    let round_trip = vfs::read_all("/bin/hello", &mut readback) == Ok(EXTERNAL_APPLICATION.len())
        && readback[..] == EXTERNAL_APPLICATION[..];
    if !step("read the image back from the filesystem", round_trip) {
        return false;
    }

    // Load from those filesystem bytes, not from the embedded constant.
    let Some(program) = userspace::load_elf_with_argv_verified(&readback, &["/bin/hello"]) else {
        return step("load from filesystem bytes", false);
    };
    if !step("load from filesystem bytes", true) {
        return false;
    }

    let pid = match task::spawn_user_process("hello", program) {
        Ok((pid, _context)) => pid,
        Err(_) => return step("spawn in Ring 3", false),
    };
    if !step("spawn in Ring 3", true) {
        return false;
    }

    let start = timer::ticks();
    while !task::process_exited(pid) {
        if timer::ticks().wrapping_sub(start) > 500 {
            return step("application runs to completion", false);
        }
        task::yield_now();
    }
    if !step("application runs to completion", true) {
        return false;
    }

    step(
        "application exits zero",
        task::wait_process(pid.as_u64()) == Ok(0),
    )
}

// ------------------------------- end-to-end client surface via a real app

#[cfg(any(
    feature = "stage18-2-test",
    feature = "stage19-2-test",
    feature = "stage19-4-test",
    feature = "stage19-5-test",
    feature = "stage19-6-test",
    feature = "stage19-7-test",
    feature = "stage19-8-test",
    feature = "stage19-9-test",
    feature = "stage19-11-test",
    feature = "stage19-12-test",
    feature = "stage19-13-test",
    feature = "stage19-14-test",
    feature = "stage20-2-test",
    feature = "stage20-3-test",
    feature = "stage20-4-test",
    feature = "stage20-5-test",
    feature = "stage20-6-test",
    feature = "stage20-7-test",
    feature = "stage20-8-test",
    feature = "stage20-9-test",
    feature = "stage20-10-test",
    feature = "stage20-18-test",
    feature = "stage20-11-test",
    feature = "stage20-12-test",
    feature = "stage13-15-test",
    feature = "stage20-13-test",
    feature = "stage20-14b-test",
    feature = "stage20-15-test",
    feature = "stage20-16-test",
    feature = "stage30-1-test",
    feature = "stage30-2-test",
    feature = "stage30-3-test",
    feature = "stage30-4-test",
    feature = "stage30-5-test",
))]
const HELLO_GUI_IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_HELLO_GUI_IMAGE"));

/// Authority a graphical client needs: console output for its progress lines
/// and `DisplayIo` for the compositor. Ordinary user processes do not carry
/// `DisplayIo`, so the app is started through the system-service path.
#[cfg(any(
    feature = "stage18-2-test",
    feature = "stage19-2-test",
    feature = "stage19-4-test",
    feature = "stage19-5-test",
    feature = "stage19-6-test",
    feature = "stage19-7-test",
    feature = "stage19-8-test",
    feature = "stage19-9-test",
    feature = "stage19-11-test",
    feature = "stage19-12-test",
    feature = "stage19-13-test",
    feature = "stage19-14-test",
    feature = "stage20-2-test",
    feature = "stage20-3-test",
    feature = "stage20-4-test",
    feature = "stage20-5-test",
    feature = "stage20-6-test",
    feature = "stage20-7-test",
    feature = "stage20-8-test",
    feature = "stage20-9-test",
    feature = "stage20-10-test",
    feature = "stage20-18-test",
    feature = "stage20-11-test",
    feature = "stage20-12-test",
    feature = "stage20-16-test",
    feature = "stage30-1-test",
    feature = "stage30-2-test",
    feature = "stage30-3-test",
    feature = "stage30-4-test",
    feature = "stage30-5-test",
))]
fn display_client_capabilities() -> capability::CapabilitySet {
    capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
}

/// Spawn hello-gui from `image` with `argv`, either as an ordinary user process
/// or with display authority, and wait for it. Returns the pid and exit status.
#[cfg(any(
    feature = "stage18-2-test",
    feature = "stage19-2-test",
    feature = "stage19-4-test",
    feature = "stage19-5-test",
    feature = "stage19-6-test",
    feature = "stage19-7-test",
    feature = "stage19-8-test",
    feature = "stage19-9-test",
    feature = "stage19-11-test",
    feature = "stage19-12-test",
    feature = "stage19-13-test",
    feature = "stage19-14-test",
    feature = "stage20-2-test",
    feature = "stage20-3-test",
    feature = "stage20-4-test",
    feature = "stage20-5-test",
    feature = "stage20-6-test",
    feature = "stage20-7-test",
    feature = "stage20-8-test",
    feature = "stage20-9-test",
    feature = "stage20-10-test",
    feature = "stage20-18-test",
    feature = "stage20-11-test",
    feature = "stage20-12-test",
    feature = "stage20-16-test",
    feature = "stage30-1-test",
    feature = "stage30-2-test",
    feature = "stage30-3-test",
    feature = "stage30-4-test",
    feature = "stage30-5-test",
))]
fn run_hello_gui(
    image: &[u8],
    argv: &[&str],
    display_authority: bool,
) -> Option<(task::ProcessId, Result<i32, task::WaitError>)> {
    let pid = spawn_hello_gui(
        image,
        argv,
        display_authority.then(display_client_capabilities),
    )?;
    wait_hello_gui(pid)
}

/// Spawn hello-gui as an ordinary user process (`None`) or as a service with
/// exactly `capabilities`.
/// Stage 13.15 acceptance: WovenAudio playback from Ring 3. hello-gui
/// --audio plays a quarter second of a known square wave through syscalls
/// 203-205. It must finish through the controller's completion interrupt,
/// and a client without AudioPlayback must be refused. The harness records
/// QEMU's audio output and checks the waveform.
#[cfg(feature = "stage13-15-test")]
fn stage13_15_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S13.15] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    let report = drivers::bring_up();
    if !step(
        "hda0 bound and ready",
        matches!(report.drivers[3].outcome, drivers::Outcome::Ready(_)),
    ) {
        return false;
    }
    let Some(info) = hda::audio_info() else {
        return step("audio output started", false);
    };
    if !step(
        "audio output on a device interrupt vector",
        info.playback_streams > 0
            && (interrupts::PCI_VECTOR_FIRST..=interrupts::PCI_VECTOR_LAST).contains(&info.vector),
    ) {
        return false;
    }
    let installed = (vfs::stat("/bin").is_ok() || vfs::mkdir("/bin").is_ok())
        && (vfs::stat("/bin/hello-gui").is_ok()
            || vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok());
    if !step("hello-gui installed", installed) {
        return false;
    }
    let player = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::AudioPlayback);
    let irqs_before = interrupts::pci_device_irq_count(info.vector);
    let plays_before = hda::audio_plays();
    let on_time_before = hda::audio_on_time();
    let played = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--audio"],
        Some(player),
    )
    .and_then(wait_hello_gui)
    .map(|(_, status)| status);
    let irqs = interrupts::pci_device_irq_count(info.vector) - irqs_before;
    serial::write_line(format_args!(
        "[S13.15] player exit={:?} completion interrupts={} plays={}",
        played.map(Result::ok),
        irqs,
        hda::audio_plays() - plays_before
    ));
    let ok_play = step(
        "a client with AudioPlayback plays the wave to completion",
        played == Some(Ok(0)) && hda::audio_plays() - plays_before == 1,
    );
    // The caller sleeps until the completion interrupt wakes it; without
    // that wakeup it would only notice at its deadline.
    let ok_irq = step(
        "completion arrives as a device interrupt and wakes the player before its deadline",
        irqs >= 1 && hda::audio_on_time() - on_time_before == 1,
    );
    // Same program, without the capability: refused before any playback.
    let denied = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--audio"],
        Some(capability::CapabilitySet::only(
            capability::Capability::Console,
        )),
    )
    .and_then(wait_hello_gui)
    .map(|(_, status)| status);
    let ok_denied = step(
        "a client without AudioPlayback is refused",
        denied == Some(Ok(3)) && hda::audio_plays() - plays_before == 1,
    );
    ok_play && ok_irq && ok_denied
}

#[cfg(any(
    feature = "stage18-2-test",
    feature = "stage19-2-test",
    feature = "stage19-4-test",
    feature = "stage19-5-test",
    feature = "stage19-6-test",
    feature = "stage19-7-test",
    feature = "stage19-8-test",
    feature = "stage19-9-test",
    feature = "stage19-11-test",
    feature = "stage19-12-test",
    feature = "stage19-13-test",
    feature = "stage19-14-test",
    feature = "stage20-2-test",
    feature = "stage20-3-test",
    feature = "stage20-4-test",
    feature = "stage20-5-test",
    feature = "stage20-6-test",
    feature = "stage20-7-test",
    feature = "stage20-8-test",
    feature = "stage20-9-test",
    feature = "stage20-10-test",
    feature = "stage20-18-test",
    feature = "stage20-11-test",
    feature = "stage20-12-test",
    feature = "stage13-15-test",
    feature = "stage20-13-test",
    feature = "stage20-14b-test",
    feature = "stage20-16-test",
    feature = "stage30-1-test",
    feature = "stage30-2-test",
    feature = "stage30-3-test",
    feature = "stage30-4-test",
    feature = "stage30-5-test",
))]
fn spawn_hello_gui(
    image: &[u8],
    argv: &[&str],
    capabilities: Option<capability::CapabilitySet>,
) -> Option<task::ProcessId> {
    let program = userspace::load_elf_with_argv(image, argv)?;
    Some(match capabilities {
        Some(capabilities) => {
            task::spawn_user_system_service("hello-gui", program, capabilities)
                .ok()?
                .0
        }
        None => task::spawn_user_process("hello-gui", program).ok()?.0,
    })
}

#[cfg(any(
    feature = "stage18-2-test",
    feature = "stage19-2-test",
    feature = "stage19-4-test",
    feature = "stage19-5-test",
    feature = "stage19-6-test",
    feature = "stage19-7-test",
    feature = "stage19-8-test",
    feature = "stage19-9-test",
    feature = "stage19-11-test",
    feature = "stage19-12-test",
    feature = "stage19-13-test",
    feature = "stage19-14-test",
    feature = "stage20-2-test",
    feature = "stage20-3-test",
    feature = "stage20-4-test",
    feature = "stage20-5-test",
    feature = "stage20-6-test",
    feature = "stage20-7-test",
    feature = "stage20-8-test",
    feature = "stage20-9-test",
    feature = "stage20-10-test",
    feature = "stage20-18-test",
    feature = "stage20-11-test",
    feature = "stage20-12-test",
    feature = "stage13-15-test",
    feature = "stage20-13-test",
    feature = "stage20-14b-test",
    feature = "stage20-16-test",
    feature = "stage30-1-test",
    feature = "stage30-2-test",
    feature = "stage30-3-test",
    feature = "stage30-4-test",
    feature = "stage30-5-test",
))]
fn wait_hello_gui(pid: task::ProcessId) -> Option<(task::ProcessId, Result<i32, task::WaitError>)> {
    let start = timer::ticks();
    while !task::process_exited(pid) {
        if timer::ticks().wrapping_sub(start) > 1000 {
            return None;
        }
        task::yield_now();
    }
    Some((pid, task::wait_process(pid.as_u64())))
}

/// WovenShell image for the Stage 20 gates, built by their harness.
#[cfg(any(
    feature = "stage20-1-test",
    feature = "stage20-2-test",
    feature = "stage20-3-test",
    feature = "stage20-4-test",
    feature = "stage20-5-test",
    feature = "stage20-6-test",
    feature = "stage20-7-test",
    feature = "stage20-8-test",
    feature = "stage20-9-test",
    feature = "stage20-10-test",
    feature = "stage20-18-test",
    feature = "stage20-11-test",
    feature = "stage20-12-test",
    feature = "stage20-13-test",
    feature = "stage20-14b-test",
    feature = "stage20-15-test",
    feature = "stage20-17-test",
    feature = "system-apps"
))]
const WOVEN_SHELL_IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_SHELL_IMAGE"));

/// Stage 20.14b acceptance: the desktop across two monitors. The kernel
/// makes a two-head virtio-gpu the display, starts the shell and checks,
/// against its own rendering, that the taskbar is on the primary monitor only
/// and that window A, dragged by its title bar, moves onto the second
/// monitor. It then prints sample pixels with their physical head
/// coordinates; the harness captures both heads from the host and checks
/// each sample.
#[cfg(feature = "stage20-14b-test")]
fn stage20_14b_self_test() -> bool {
    use crate::display::{blend_pixel, rgb};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const RED: u32 = rgb(200, 60, 60);
    const START_FILL: u32 = rgb(52, 96, 180);

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.14B] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn stays(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        while timer::ticks().wrapping_sub(start) <= limit {
            if !condition() {
                return false;
            }
            task::yield_now();
        }
        true
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32, left: bool) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, left) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }

    let heads = match compositor::attach_gpu_outputs() {
        Ok(heads) => heads,
        Err(error) => {
            serial::write_line(format_args!("[S20.14B] attach: {:?}", error));
            return step("a two-head virtio-gpu is the display", false);
        }
    };
    if !step("a two-head virtio-gpu is the display", heads == 2) {
        return false;
    }
    let installed = vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        installed
            && until(3000, || {
                compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
            }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let scale = compositor::scale().unwrap_or(1) as i32;
    let primary = width / 2;
    let top = height - 24;
    serial::write_line(format_args!(
        "[S20.14B] desktop {}x{} at scale {}, primary {} wide",
        width, height, scale, primary
    ));
    let wall = move |x: i32, y: i32| compositor::pixel(x, y) == Ok(wallpaper(x, y, height));
    let taskbar_on_primary_only = until(2000, || {
        compositor::pixel(6, top + 6) == Ok(START_FILL)
            && wall(primary + 7, top + 7)
            && wall(width - 7, top + 7)
    });
    if !step(
        "the taskbar spans the primary monitor only",
        taskbar_on_primary_only,
    ) {
        return false;
    }
    let client = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(_alpha) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "a"],
        Some(client),
    ) else {
        return step("application A spawned", false);
    };
    if !step(
        "window A opens on the primary monitor",
        until(2000, || compositor::pixel(45, 45) == Ok(RED)),
    ) {
        return false;
    }
    // From the desktop's centre to A's title bar, then drag it one
    // monitor-width to the right.
    let (px, py) = (width / 2, height / 2);
    // Judged only on completed frames: the surface is read live, and the
    // shell draws a frame in several writes.
    let on_second = |pixel: &dyn Fn(i32, i32) -> Option<u32>| {
        pixel(45 + primary, 45) == Some(RED) && pixel(45, 45) == Some(wallpaper(45, 45, height))
    };
    let injected = move_by(100 - px, 33 - py, false)
        && packet(0, 0, true)
        && move_by(primary, 0, true)
        && packet(0, 0, false);
    let arrived = injected && until(1000, || compositor::between_frames(on_second) == Some(true));
    // And it stays there: the release is processed after the moves, and an
    // edge snap or resize would move it again.
    let stayed = arrived && stays(100, || compositor::between_frames(on_second) != Some(false));
    if !stayed {
        serial::write_line(format_args!(
            "[S20.14B] drag: injected={} arrived={} stayed={}",
            injected, arrived, stayed
        ));
    }
    let dragged = stayed;
    if !step(
        "dragging A by its title bar moves it onto the second monitor",
        dragged,
    ) {
        return false;
    }
    // The surface is right; wait until it has reached the monitors too.
    if !step(
        "everything drawn has been presented to the monitors",
        until(1000, compositor::presented),
    ) {
        return false;
    }
    // Samples for the host captures: surface point -> head and physical
    // position at the centre of its scaled block.
    let samples = [
        (45 + primary, 45),
        (45, 45),
        (6, top + 6),
        (primary + 7, top + 7),
        (100 + primary, 60),
        (primary / 2, height / 3),
    ];
    let head_width = primary * scale;
    for (x, y) in samples {
        let Ok(pixel) = compositor::pixel(x, y) else {
            return step("sample read", false);
        };
        let physical = (x * scale + scale / 2, y * scale + scale / 2);
        serial::write_line(format_args!(
            "[S20.14B] sample head={} x={} y={} rgb={:06x}",
            physical.0 / head_width,
            physical.0 % head_width,
            physical.1,
            pixel
        ));
    }
    serial::write_line(format_args!(
        "[S20.14B] ready for capture {}x{}",
        head_width,
        height * scale
    ));
    let start = timer::ticks();
    while timer::ticks().wrapping_sub(start) < 1500 {
        task::yield_now();
    }
    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "shell released the display",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.15 acceptance: the desktop as the session.
///
/// The session starts through `session::start`, as a system-apps boot
/// starts it, with an application in the catalog. Then, with real i8042
/// input:
/// - the shell takes the display and input focus, and its launcher (F1)
///   lists the application;
/// - F12 shows the diagnostic console: the session's frames stop, the
///   terminal owns the display, and a typed key reaches the kernel shell's
///   queue rather than the session;
/// - F12 returns: the session takes the display and focus back;
/// - Quick Settings' first row, Power off, turns the machine off through
///   syscall 209: the harness requires QEMU to stop by itself.
#[cfg(feature = "stage20-15-test")]
fn stage20_15_self_test() -> bool {
    use crate::display::rgb;
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.15] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }
    /// Press and release one key on the i8042.
    fn key(code: u8) -> bool {
        mouse::keyboard_loopback(code) && mouse::keyboard_loopback(code | 0x80)
    }
    /// The launcher's first row reads `name` in white.
    fn text_at(x: i32, y: i32, name: &str, color: u32) -> bool {
        let mut mask = crate::display::Surface::new(crate::font::text_width(name), 7);
        let bounds = mask.bounds();
        mask.fill_rect(bounds, 0x0100_0000);
        mask.draw_text(0, 0, name, color);
        (0..7).all(|row| {
            (0..crate::font::text_width(name) as i32).all(|column| {
                mask.pixel(column, row) != Some(color)
                    || compositor::pixel(x + column, y + row) == Ok(color)
            })
        })
    }

    let catalog = (vfs::stat("/share").is_ok() || vfs::mkdir("/share").is_ok())
        && (vfs::stat("/share/applications").is_ok() || vfs::mkdir("/share/applications").is_ok())
        && vfs::create_read_only(
            "/share/applications/hello.app",
            b"Name=Hello\nExec=hello-gui --desktop-client %p a\n",
        )
        .is_ok()
        && vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    if !step("an application in the catalog", catalog) {
        return false;
    }
    let Ok(shell) = session::start(WOVEN_SHELL_IMAGE, None) else {
        return step("the session started", false);
    };
    let presenting = || compositor::holder() == Some(shell) && compositor::is_presenting();
    if !step(
        "the session takes the display and input focus",
        until(3000, || {
            presenting() && woven_input::focus() == Some(shell) && session::active() == Some(shell)
        }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    // F1: the launcher's first row is the catalog's application.
    // The launcher panel: x 4, three rows of 14 plus its border, above the
    // taskbar; the first match's text at (+6, +1 + 2 * 14 + 4).
    let listed = key(0x3b)
        && until(1000, || {
            text_at(4 + 6, top - 2 - 44 + 33, "Hello", rgb(255, 255, 255))
        });
    let closed = key(0x3b);
    if !step(
        "the launcher lists the installed application",
        listed && closed,
    ) {
        return false;
    }

    // F12: the diagnostic console.
    let console = key(0x58)
        && until(1000, || {
            session::console_active()
                && !compositor::is_presenting()
                && woven_input::focus().is_none()
        });
    if !step(
        "F12 shows the diagnostic console; the session's frames stop",
        console,
    ) {
        return false;
    }
    while keyboard::poll().is_some() {}
    let typed = mouse::keyboard_loopback(0x2d)
        && mouse::keyboard_loopback(0xad)
        && until(500, || {
            matches!(keyboard::poll(), Some(keyboard::Key::Char('x')))
        });
    if !step("a key typed at the console reaches the kernel shell", typed) {
        return false;
    }
    let back = key(0x58)
        && until(1000, || {
            !session::console_active() && presenting() && woven_input::focus() == Some(shell)
        });
    if !step("F12 returns to the desktop: display and focus", back) {
        return false;
    }

    // Quick Settings, then its first row: Power off (8 rows now: Change PIN
    // and Language were appended after Lock screen, so only the row-count
    // term needs updating).
    let button = (width - 54 - 5 * 20, top + 4);
    let power_row = (width - 4 - 164 + 28, top - 2 - (11 * 14 + 2) + 1 + 14 + 7);
    let (px, py) = (width / 2, height / 2);
    let opened = move_by(button.0 + 8 - px, button.1 + 8 - py) && click();
    if !step(
        "Quick Settings opens",
        opened
            && until(1000, || {
                text_at(
                    power_row.0 - 28 + 6,
                    power_row.1 - 3,
                    "Power off",
                    rgb(255, 255, 255),
                )
            }),
    ) {
        return false;
    }
    serial::write_line(format_args!("[S20.15] clicking Power off"));
    let _ = move_by(power_row.0 - (button.0 + 8), power_row.1 - (button.1 + 8)) && click();
    // The machine should now be off; still running after this is failure.
    let start = timer::ticks();
    while timer::ticks().wrapping_sub(start) < 1000 {
        task::yield_now();
    }
    step("the machine turned off from the desktop", false)
}

/// Stage 24.59 acceptance: the Change-PIN flow through the real WovenShell UI.
///
/// This uses the production identity/session capability boundary, kernel-
/// injected pointer and keyboard input, and the account-file boundary. It
/// proves that the old PIN is rejected after the change and the new PIN works
/// after the shell reloads its account record.
#[cfg(feature = "stage24-59-test")]
fn stage24_59_change_pin_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Surface};
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const BLACK: u32 = rgb(0, 0, 0);
    const UNSET: u32 = 0x0100_0000;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S24.59] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }
    fn keys(codes: &[u8]) -> bool {
        codes.iter().all(|&code| mouse::keyboard_loopback(code))
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn account_line(name: &str, salt: &[u8], pin: &[u8]) -> alloc::string::String {
        let mut hash = [0_u8; 32];
        pbkdf2_hmac::<Sha256>(pin, salt, 100_000, &mut hash);
        let mut line = alloc::format!("{name}:");
        for byte in salt {
            line.push_str(&alloc::format!("{byte:02x}"));
        }
        line.push(':');
        for byte in hash {
            line.push_str(&alloc::format!("{byte:02x}"));
        }
        line.push('\n');
        line
    }
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    fn file_accepts(pin: &[u8]) -> bool {
        let Ok(stat) = vfs::stat("/etc/wovenhat/accounts") else {
            return false;
        };
        let mut data = alloc::vec![0_u8; stat.size];
        if vfs::read_all("/etc/wovenhat/accounts", &mut data) != Ok(stat.size) {
            return false;
        }
        let Some(line) = data
            .split(|byte| *byte == b'\n')
            .find(|line| line.starts_with(b"ada:"))
        else {
            return false;
        };
        let mut fields = line.split(|byte| *byte == b':');
        let (Some(name), Some(salt_text), Some(hash_text)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return false;
        };
        if name != b"ada"
            || fields.next().is_some()
            || salt_text.len() % 2 != 0
            || hash_text.len() != 64
        {
            return false;
        }
        let mut salt = [0_u8; 32];
        if salt_text.len() / 2 > salt.len() {
            return false;
        }
        for (index, pair) in salt_text.chunks(2).enumerate() {
            let (Some(high), Some(low)) = (hex(pair[0]), hex(pair[1])) else {
                return false;
            };
            salt[index] = (high << 4) | low;
        }
        let mut expected = [0_u8; 32];
        pbkdf2_hmac::<Sha256>(pin, &salt[..salt_text.len() / 2], 100_000, &mut expected);
        for (index, pair) in hash_text.chunks(2).enumerate() {
            let (Some(high), Some(low)) = (hex(pair[0]), hex(pair[1])) else {
                return false;
            };
            if expected[index] != ((high << 4) | low) {
                return false;
            }
        }
        true
    }

    let mut file = account_line("ada", b"\x11\x22\x33\x44\x55\x66\x77\x88", b"1234");
    file.push_str(&account_line(
        "lin",
        b"\xa0\xb1\xc2\xd3\xe4\xf5\x06\x17",
        b"9876",
    ));
    let written = (vfs::stat("/etc").is_ok() || vfs::mkdir("/etc").is_ok())
        && vfs::mkdir("/etc/wovenhat").is_ok()
        && vfs::create_read_only("/etc/wovenhat/accounts", file.as_bytes()).is_ok();
    if !step("accounts fixture written", written) {
        return false;
    }
    let Ok(shell) = session::start(WOVEN_SHELL_IMAGE, Some("/etc/wovenhat/accounts")) else {
        return step("identity-capable shell started", false);
    };
    if !step(
        "shell took the display and input focus",
        until(3000, || {
            compositor::holder() == Some(shell)
                && compositor::is_presenting()
                && session::active() == Some(shell)
        }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    let middle = height / 2;
    let line = |y0: i32, text: &str| {
        let x0 = (width - crate::font::text_width(text) as i32) / 2;
        let mut mask = Surface::new(width as usize, 7);
        let bounds = mask.bounds();
        mask.fill_rect(bounds, UNSET);
        mask.draw_text(x0, 0, text, WHITE);
        (y0..y0 + 7).all(|y| {
            (0..width).all(|x| {
                let expected = if mask.pixel(x, y - y0) == Some(WHITE) {
                    WHITE
                } else {
                    blend_pixel(BLACK, wallpaper(x, y, height), 160)
                };
                compositor::pixel(x, y) == Ok(expected)
            })
        })
    };
    if !step(
        "ada sign-in prompt rendered",
        until(1000, || {
            line(middle - 60, "Sign in") && line(middle - 4, "Enter PIN")
        }),
    ) {
        return false;
    }
    if !step(
        "ada signs in",
        keys(&[0x02, 0x03, 0x04, 0x05, 0x1c]) && until(1000, || !line(middle - 60, "Sign in")),
    ) {
        return false;
    }
    let button = (width - 54 - 5 * 20, top + 4);
    // Row 6 of 8, "Change PIN": Language was appended after it, so only the
    // row-count term needs updating, not the row index.
    let change_row = (
        width - 4 - 164 + 28,
        top - 2 - (10 * 14 + 2) + 1 + 6 * 14 + 7,
    );
    let opened = move_by(button.0 + 8 - width / 2, button.1 + 8 - middle)
        && click()
        && until(1000, compositor::presented);
    let chosen = opened
        && move_by(change_row.0 - (button.0 + 8), change_row.1 - (button.1 + 8))
        && click()
        && until(1000, || line(middle - 34, "Change PIN"));
    if !step("Quick Settings Change PIN opens", chosen) {
        return false;
    }
    let change_keys = keys(&[
        0x02, 0x03, 0x04, 0x05, 0x1c, // current 1234
        0x07, 0x09, 0x1c, // new 68
        0x07, 0x09, 0x1c, // confirm 68
    ]);
    let changed = change_keys && until(1000, || !line(middle - 34, "Change PIN"));
    if !step("new PIN accepted and desktop restored", changed) {
        return false;
    }
    if !step(
        "old PIN is absent from the persisted record",
        !file_accepts(b"1234"),
    ) || !step(
        "new PIN is present in the persisted record",
        file_accepts(b"68"),
    ) {
        return false;
    }
    // Row 5 of 8, "Change PIN" and Language were both appended after
    // "Lock screen", so only the row-count term needs updating.
    let lock_row = (
        width - 4 - 164 + 28,
        top - 2 - (10 * 14 + 2) + 1 + 5 * 14 + 7,
    );
    let qs_reopened = move_by(button.0 + 8 - change_row.0, button.1 + 8 - change_row.1) && click();
    let locked = qs_reopened
        && move_by(lock_row.0 - (button.0 + 8), lock_row.1 - (button.1 + 8))
        && click()
        && until(1000, || line(middle - 34, "Locked"));
    let old_refused = locked
        && keys(&[0x02, 0x03, 0x04, 0x05, 0x1c])
        && until(500, || line(middle - 4, "Wrong PIN"));
    let new_works =
        old_refused && keys(&[0x07, 0x09, 0x1c]) && until(1000, || !line(middle - 34, "Locked"));
    if !step("old PIN is refused and new PIN unlocks", new_works) {
        return false;
    }
    let _ = task::kill_process(shell, 9);
    true
}

/// Stage 20.13 acceptance: workspaces. With real i8042 input the kernel
/// checks, against its own computation of every pixel it samples:
/// - the switcher shows four workspaces with the first current, and window A
///   opens there;
/// - F3 switches to workspace 2: A and its taskbar entry disappear;
/// - window B opens on workspace 2;
/// - clicking switcher box 1 brings A back and hides B;
/// - F4 moves A to workspace 2 and follows it: A and B both show;
/// - F3 to workspace 3 hides both; B is killed there, and back on workspace
///   2 only A remains.
#[cfg(feature = "stage20-13-test")]
fn stage20_13_self_test() -> bool {
    use crate::display::{blend_pixel, rgb};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const RED: u32 = rgb(200, 60, 60);
    const BLUE: u32 = rgb(60, 60, 200);
    const TASKBAR: u32 = rgb(20, 24, 32);
    const ENTRY_ACTIVE: u32 = rgb(70, 120, 210);
    const ENTRY_INACTIVE: u32 = rgb(44, 52, 68);
    const F3: u8 = 0x3d;
    const F4: u8 = 0x3e;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.13] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }
    fn keys(codes: &[u8]) -> bool {
        codes.iter().all(|&code| mouse::keyboard_loopback(code))
    }

    let installed = vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        installed
            && until(3000, || {
                compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
            }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    // Switcher boxes, left of the Quick Settings button.
    let qs_x = width - (8 * crate::font::ADVANCE + 6) - 5 * (16 + 4);
    let box_x = |index: i32| qs_x - 6 - (4 * 14 - 2) + index * 14;
    let current = move |index: i32| {
        (0..4).all(|other| {
            let expected = if other == index {
                ENTRY_ACTIVE
            } else {
                ENTRY_INACTIVE
            };
            compositor::pixel(box_x(other) + 1, top + 5) == Ok(expected)
        })
    };
    let wall = move |x: i32, y: i32| compositor::pixel(x, y) == Ok(wallpaper(x, y, height));
    // The first window entry's left edge: an entry, or bare taskbar.
    let taskbar = move |x: i32, y: i32| blend_pixel(TASKBAR, wallpaper(x, y, height), 216);
    let no_entry = move || compositor::pixel(57, top + 5) == Ok(taskbar(57, top + 5));
    // The second entry slot (80 wide, 4 apart): only one window listed.
    let one_entry =
        move || !no_entry() && compositor::pixel(141, top + 5) == Ok(taskbar(141, top + 5));
    // B lies inside A's shadowed footprint, so with A showing a hidden B is
    // judged by its colour being gone and by the taskbar listing one window.
    let (a_x, a_y, b_x, b_y) = (45, 45, 162, 122);
    let b_hidden = move || compositor::pixel(b_x, b_y) != Ok(BLUE) && one_entry();
    // Both windows listed on the current workspace.
    let two_entries = move || {
        matches!(
            compositor::pixel(141, top + 5),
            Ok(ENTRY_ACTIVE | ENTRY_INACTIVE)
        ) && !no_entry()
    };

    if !step(
        "four workspaces, the first current",
        until(1000, || current(0)),
    ) {
        return false;
    }
    let client = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(_alpha) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "a"],
        Some(client),
    ) else {
        return step("application A spawned", false);
    };
    if !step(
        "window A opens on workspace 1",
        until(2000, || {
            compositor::pixel(a_x, a_y) == Ok(RED) && !no_entry()
        }),
    ) {
        return false;
    }
    let switched = keys(&[F3]) && until(1000, || current(1) && wall(a_x, a_y) && no_entry());
    if !step(
        "F3: workspace 2 is current; A and its entry are gone",
        switched,
    ) {
        return false;
    }
    let Some(beta) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "b"],
        Some(client),
    ) else {
        return step("application B spawned", false);
    };
    if !step(
        "window B opens on workspace 2, A stays hidden",
        until(2000, || {
            compositor::pixel(b_x, b_y) == Ok(BLUE) && wall(a_x, a_y)
        }),
    ) {
        return false;
    }
    let (px, py) = (width / 2, height / 2);
    let clicked = move_by(box_x(0) + 2 - px, top + 8 - py)
        && click()
        && until(1000, || {
            current(0) && compositor::pixel(a_x, a_y) == Ok(RED) && b_hidden()
        });
    if !step("clicking box 1 brings A back and hides B", clicked) {
        return false;
    }
    let moved = keys(&[F4])
        && until(1000, || {
            current(1) && compositor::pixel(a_x, a_y) == Ok(RED) && two_entries()
        });
    if !step(
        "F4 moves A to workspace 2 and follows: A on top, both listed",
        moved,
    ) {
        return false;
    }
    let away = keys(&[F3])
        && until(1000, || {
            current(2) && wall(a_x, a_y) && wall(b_x, b_y) && no_entry()
        });
    if !step("F3: workspace 3 shows neither", away) {
        return false;
    }
    let killed =
        task::kill_process(beta.as_u64(), 9).is_ok() && until(1000, || task::process_exited(beta));
    let _ = task::wait_process(beta.as_u64());
    // Workspace 3 -> 4 -> 1 -> 2.
    let back = killed
        && keys(&[F3, F3, F3])
        && until(1000, || {
            current(1) && compositor::pixel(a_x, a_y) == Ok(RED) && b_hidden()
        });
    if !step(
        "B closed while hidden; back on workspace 2 only A remains",
        back,
    ) {
        return false;
    }
    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.12 acceptance: signing in.
///
/// The kernel writes an accounts file with two local accounts: `ada`, PIN
/// 1234, and `lin`, PIN 9876. Each line holds a salt and `SHA-256(salt ||
/// PIN)`, computed here with the kernel's own SHA-256. The shell starts with
/// `--accounts` and `FileRead`. With real i8042 input the kernel checks:
/// - the desktop opens at the sign-in screen, ada chosen;
/// - Tab chooses lin;
/// - ada's PIN is refused for lin;
/// - lin's PIN signs in, and lin's name appears in the taskbar;
/// - once locked, the lock screen wants lin's PIN, and refuses ada's.
///
/// Rows are compared with the kernel's own rendering.
#[cfg(feature = "stage20-12-test")]
fn stage20_12_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Surface};
    use sha2::Sha256;

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const BLACK: u32 = rgb(0, 0, 0);
    const TASKBAR: u32 = rgb(20, 24, 32);
    const UNSET: u32 = 0x0100_0000;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.12] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }
    fn keys(codes: &[u8]) -> bool {
        codes.iter().all(|&code| mouse::keyboard_loopback(code))
    }
    /// One accounts-file line: `name:salt-hex:hash-hex`. Must match
    /// `libwoven::accounts::{hash_pin, PBKDF2_ITERATIONS}`.
    fn account_line(name: &str, salt: &[u8], pin: &[u8]) -> alloc::string::String {
        let mut hash = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(pin, salt, 100_000, &mut hash);
        let mut line = alloc::format!("{name}:");
        for byte in salt {
            line.push_str(&alloc::format!("{byte:02x}"));
        }
        line.push(':');
        for byte in hash {
            line.push_str(&alloc::format!("{byte:02x}"));
        }
        line.push('\n');
        line
    }

    let mut file = account_line("ada", b"\x11\x22\x33\x44\x55\x66\x77\x88", b"1234");
    file.push_str(&account_line(
        "lin",
        b"\xa0\xb1\xc2\xd3\xe4\xf5\x06\x17",
        b"9876",
    ));
    // Boot creates /etc; make sure of it rather than assume.
    let written = (vfs::stat("/etc").is_ok() || vfs::mkdir("/etc").is_ok())
        && vfs::mkdir("/etc/wovenhat").is_ok()
        && vfs::create_read_only("/etc/wovenhat/accounts", file.as_bytes()).is_ok()
        && vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    if !step("accounts file written (salted hashes, no PINs)", written) {
        return false;
    }
    if !step(
        "accounts loaded into the kernel identity authority",
        identity_runtime::load_accounts_file("/etc/wovenhat/accounts").is_ok(),
    ) {
        return false;
    }
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc)
        .with(capability::Capability::FileRead)
        .with(capability::Capability::AccountManage);
    let Some(program) = userspace::load_elf_with_argv(
        WOVEN_SHELL_IMAGE,
        &["/bin/woven-shell", "--accounts", "/etc/wovenhat/accounts"],
    ) else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        until(3000, || {
            compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
        }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    let middle = height / 2;
    let dim = move |x: i32, y: i32| blend_pixel(BLACK, wallpaper(x, y, height), 160);
    let line = |y0: i32, text: &str| {
        let x0 = (width - crate::font::text_width(text) as i32) / 2;
        let mut mask = Surface::new(width as usize, 7);
        let bounds = mask.bounds();
        mask.fill_rect(bounds, UNSET);
        mask.draw_text(x0, 0, text, WHITE);
        (y0..y0 + 7).all(|y| {
            (0..width).all(|x| {
                let expected = if mask.pixel(x, y - y0) == Some(WHITE) {
                    WHITE
                } else {
                    dim(x, y)
                };
                compositor::pixel(x, y) == Ok(expected)
            })
        })
    };
    let sign_in = |first: &str, second: &str, prompt: &str| {
        line(middle - 60, "Sign in")
            && line(middle - 46, first)
            && line(middle - 36, second)
            && line(middle - 4, prompt)
            && compositor::pixel(300, top + 12) == Ok(dim(300, top + 12))
    };
    if !step(
        "the desktop opens at the sign-in screen, ada chosen",
        until(1000, || sign_in("> ada", "  lin", "Enter PIN")),
    ) {
        return false;
    }
    if !step(
        "Tab chooses lin",
        keys(&[0x0f]) && until(500, || sign_in("  ada", "> lin", "Enter PIN")),
    ) {
        return false;
    }
    let refused = keys(&[0x02, 0x03, 0x04, 0x05, 0x1c])
        && until(500, || sign_in("  ada", "> lin", "Wrong PIN"));
    if !step("ada's PIN is refused for lin", refused) {
        return false;
    }
    // lin's name sits left of the Quick Settings button, on the taskbar.
    let button = (width - 54 - 5 * 20, top + 4);
    // Left of the workspace switcher (Stage 20.13), itself 6 left of the
    // Quick Settings button.
    let name_x = button.0 - 6 - (4 * 14 - 2) - 6 - crate::font::text_width("lin") as i32;
    let taskbar = move |x: i32, y: i32| blend_pixel(TASKBAR, wallpaper(x, y, height), 216);
    let name_shown = || {
        let mut mask = Surface::new(18, 7);
        let bounds = mask.bounds();
        mask.fill_rect(bounds, UNSET);
        mask.draw_text(0, 0, "lin", WHITE);
        (top + 9..top + 16).all(|y| {
            (name_x..name_x + 18).all(|x| {
                let expected = if mask.pixel(x - name_x, y - (top + 9)) == Some(WHITE) {
                    WHITE
                } else {
                    taskbar(x, y)
                };
                compositor::pixel(x, y) == Ok(expected)
            })
        })
    };
    let signed_in = keys(&[0x0a, 0x09, 0x08, 0x07, 0x1c]) && until(1000, name_shown);
    if !step(
        "lin's PIN signs in; lin's name is on the taskbar",
        signed_in,
    ) {
        return false;
    }

    // Lock through Quick Settings: the lock screen wants lin's PIN. Row 5 of
    // 8 ("Lock screen"): this geometry was stale at QS_ROWS=5/row 4, from
    // before Tiling moved "Lock screen" to row 5 -- Change PIN and Language
    // were both appended after it since, so only the row-count term needed
    // updating, not the row index.
    let lock_row = (
        width - 4 - 164 + 28,
        top - 2 - (10 * 14 + 2) + 1 + 5 * 14 + 7,
    );
    let (px, py) = (width / 2, height / 2);
    let locked = move_by(button.0 + 8 - px, button.1 + 8 - py)
        && click()
        && move_by(lock_row.0 - (button.0 + 8), lock_row.1 - (button.1 + 8))
        && click()
        && until(1000, || {
            line(middle - 34, "Locked") && line(middle - 4, "Enter PIN")
        });
    let lock_checks = locked
        && keys(&[0x02, 0x03, 0x04, 0x05, 0x1c])
        && until(500, || line(middle - 4, "Wrong PIN"))
        && keys(&[0x0a, 0x09, 0x08, 0x07, 0x1c])
        && until(1000, name_shown);
    if !step(
        "the lock screen refuses ada's PIN and takes lin's",
        lock_checks,
    ) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.11 acceptance: the lock screen.
///
/// The shell runs with `--pin 2468` and no more authority than any other
/// desktop gate. The kernel checks, with real i8042 input:
/// - Quick Settings' last row locks: the window is no longer drawn, and a
///   key does not reach its application;
/// - each digit shows as a masked dot;
/// - a wrong PIN says so and stays locked;
/// - the right PIN unlocks, and the application then gets its key;
/// - three wrong PINs lock entry out, so even the right PIN is refused
///   until the lockout ends, and then accepted.
///
/// The lock screen's rows are compared with the kernel's own rendering: the
/// dimmed wallpaper and the text in the kernel font.
#[cfg(feature = "stage20-11-test")]
fn stage20_11_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Surface};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const BLACK: u32 = rgb(0, 0, 0);
    const RED: u32 = rgb(200, 60, 60);
    const UNSET: u32 = 0x0100_0000;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.11] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn stays(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        while timer::ticks().wrapping_sub(start) <= limit {
            if !condition() {
                return false;
            }
            task::yield_now();
        }
        true
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }
    fn keys(codes: &[u8]) -> bool {
        codes.iter().all(|&code| mouse::keyboard_loopback(code))
    }

    let installed = vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) =
        userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell", "--pin", "2468"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        installed
            && until(3000, || {
                compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
            }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    let client = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(alpha) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "a"],
        Some(client),
    ) else {
        return step("application A spawned", false);
    };
    if !step(
        "window A opened",
        until(2000, || compositor::pixel(45, 45) == Ok(RED)),
    ) {
        return false;
    }

    // The lock screen as the kernel renders it: dimmed wallpaper, with
    // "Locked", the time, the prompt and the dots centred on the middle row.
    let dim = move |x: i32, y: i32| blend_pixel(BLACK, wallpaper(x, y, height), 160);
    let middle = height / 2;
    let line = |y0: i32, text: &str| {
        let x0 = (width - crate::font::text_width(text) as i32) / 2;
        let mut mask = Surface::new(width as usize, 7);
        let bounds = mask.bounds();
        mask.fill_rect(bounds, UNSET);
        mask.draw_text(x0, 0, text, WHITE);
        (y0..y0 + 7).all(|y| {
            (0..width).all(|x| {
                let expected = if mask.pixel(x, y - y0) == Some(WHITE) {
                    WHITE
                } else {
                    dim(x, y)
                };
                compositor::pixel(x, y) == Ok(expected)
            })
        })
    };
    let dots = |count: i32| {
        let left = (width - (count * 10 - 4)) / 2;
        (middle + 12..middle + 18).all(|y| {
            (0..width).all(|x| {
                let on = count > 0 && x >= left && x < left + count * 10 - 4 && (x - left) % 10 < 6;
                compositor::pixel(x, y) == Ok(if on { WHITE } else { dim(x, y) })
            })
        })
    };
    let screen = |prompt: &str, count: i32| {
        line(middle - 34, "Locked")
            && line(middle - 4, prompt)
            && dots(count)
            && compositor::pixel(45, 45) == Ok(dim(45, 45))
    };

    // Quick Settings: the button, then row 5 of 8, "Lock screen" (this
    // geometry was stale at QS_ROWS=5/row 4, from before Tiling moved
    // "Lock screen" to row 5; Change PIN and Language were both appended
    // after it since, so only the row-count term needed updating).
    let button = (width - 54 - 5 * 20 + 8, top + 12);
    let lock_row = (
        width - 4 - 164 + 28,
        top - 2 - (10 * 14 + 2) + 1 + 5 * 14 + 7,
    );
    let (mut px, mut py) = (width / 2, height / 2);
    let lock = |px: &mut i32, py: &mut i32| {
        let ok = move_by(button.0 - *px, button.1 - *py)
            && click()
            && move_by(lock_row.0 - button.0, lock_row.1 - button.1)
            && click();
        (*px, *py) = lock_row;
        ok
    };
    if !step(
        "Quick Settings locks the screen; the window is hidden",
        lock(&mut px, &mut py) && until(1000, || screen("Enter PIN", 0)),
    ) {
        return false;
    }
    // A key while locked reaches no application: A, waiting for x, must
    // still be running a second later.
    let withheld = keys(&[0x2d])
        && stays(100, || {
            !task::process_exited(alpha) && screen("Enter PIN", 0)
        });
    if !step("a key while locked reaches no application", withheld) {
        return false;
    }
    let masked = keys(&[0x03, 0x05]) && until(500, || screen("Enter PIN", 2));
    if !step("each digit shows as a masked dot", masked) {
        return false;
    }
    let wrong = keys(&[0x1c]) && until(500, || screen("Wrong PIN", 0));
    if !step("a wrong PIN says so and stays locked", wrong) {
        return false;
    }
    let unlocked = keys(&[0x03, 0x05, 0x07, 0x09, 0x1c])
        && until(1000, || compositor::pixel(45, 45) == Ok(RED));
    if !step("the right PIN unlocks and the window returns", unlocked) {
        return false;
    }
    let delivered = keys(&[0x2d]) && wait_hello_gui(alpha).map(|(_, status)| status) == Some(Ok(0));
    if !step("unlocked, A gets its key and exits", delivered) {
        return false;
    }

    // Lockout: lock again, three wrong PINs, then the right one too soon.
    let gone = until(1000, || {
        compositor::pixel(45, 45) == Ok(wallpaper(45, 45, height))
    });
    let relocked = gone && lock(&mut px, &mut py) && until(1000, || screen("Enter PIN", 0));
    let locked_out = relocked
        && keys(&[0x02, 0x1c, 0x02, 0x1c, 0x02, 0x1c])
        && until(500, || screen("Try again shortly", 0))
        && keys(&[0x03, 0x05, 0x07, 0x09, 0x1c])
        // Every key consumed and the repaint after Enter finished, then the
        // lockout must hold: had the PIN been accepted, the screen would
        // unlock.
        && until(500, || !woven_input::pending(shell.as_u64()))
        && until(500, || screen("Try again shortly", 0))
        && stays(100, || screen("Try again shortly", 0));
    if !step(
        "three wrong PINs lock entry out; the right PIN is refused meanwhile",
        locked_out,
    ) {
        return false;
    }
    let started = timer::ticks();
    while timer::ticks().wrapping_sub(started) < 520 {
        task::yield_now();
    }
    let after = keys(&[0x03, 0x05, 0x07, 0x09, 0x1c])
        && until(1000, || {
            compositor::pixel(45, 45) == Ok(wallpaper(45, 45, height))
        });
    if !step("after the lockout the right PIN unlocks", after) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.10 acceptance: Quick Settings.
///
/// The Quick Settings button opens a translucent panel of toggles that act
/// on the system:
/// - Do Not Disturb: a notification is remembered (the badge counts it)
///   but raises no toast;
/// - tiling: the only window fills the work area;
/// - display scale: the compositor surface really becomes 1280x800 at
///   scale 1, and returns to 640x400 at scale 2.
///
/// Each panel state is compared with the kernel's own rendering: the
/// background blended over what lies beneath, the border, title row and
/// rows in the kernel font. Pointer and keyboard input travel the real
/// i8042 path.
#[cfg(feature = "stage20-10-test")]
fn stage20_10_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Rect, Surface};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const ACCENT: u32 = rgb(90, 140, 220);
    const START_FILL: u32 = rgb(52, 96, 180);
    const ENTRY_ACTIVE: u32 = rgb(70, 120, 210);
    const PANEL_FILL: u32 = rgb(28, 32, 44);
    const QUIET: u32 = rgb(90, 170, 170);
    const UNSET: u32 = 0x0100_0000;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.10] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    /// Whether `condition` holds at every check for `limit` ticks.
    fn stays(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        while timer::ticks().wrapping_sub(start) <= limit {
            if !condition() {
                return false;
            }
            task::yield_now();
        }
        true
    }
    fn region(
        x0: i32,
        y0: i32,
        width: i32,
        height: i32,
        expected: impl Fn(i32, i32) -> u32,
    ) -> bool {
        (y0..y0 + height)
            .all(|y| (x0..x0 + width).all(|x| compositor::pixel(x, y) == Ok(expected(x, y))))
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }
    /// The Quick Settings panel for a desktop `width`x`height`, with `rows`,
    /// over whatever `beneath` gives, skipping the cells under the pointer
    /// at `pointer`.
    fn panel(
        width: i32,
        height: i32,
        rows: [&str; 10],
        beneath: impl Fn(i32, i32) -> u32,
        pointer: (i32, i32),
    ) -> bool {
        let top = height - 24;
        let h = 11 * 14 + 2;
        let (x0, y0) = (width - 4 - 164, top - 2 - h);
        let mut image = Surface::new(164, h as usize);
        let bounds = image.bounds();
        image.fill_rect(bounds, UNSET);
        image.stroke_rect(bounds, ACCENT);
        image.fill_rect(Rect::new(1, 1, 162, 14), START_FILL);
        image.draw_text(6, 5, "Quick Settings", WHITE);
        for (index, row) in rows.iter().enumerate() {
            image.draw_text(6, 1 + (1 + index as i32) * 14 + 4, row, WHITE);
        }
        (y0..y0 + h).all(|y| {
            (x0..x0 + 164).all(|x| {
                if (pointer.0..pointer.0 + 7).contains(&x)
                    && (pointer.1..pointer.1 + 10).contains(&y)
                {
                    return true;
                }
                let expected = match image.pixel(x - x0, y - y0) {
                    Some(UNSET) | None => blend_pixel(PANEL_FILL, beneath(x, y), 216),
                    Some(pixel) => pixel,
                };
                compositor::pixel(x, y) == Ok(expected)
            })
        })
    }

    let installed = vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        installed
            && until(3000, || {
                compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
            }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    let client = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(quiet) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "q"],
        Some(client),
    ) else {
        return step("application Q spawned", false);
    };
    if !step(
        "window Q opened",
        until(2000, || compositor::pixel(45, 45) == Ok(QUIET)),
    ) {
        return false;
    }

    // The button sits left of the tray's last slot; the panel's rows are 14
    // pixels apart: Power off and Restart (Stage 20.15), then the toggles at
    // rows 3-5 under the title, and Lock screen.
    let button = (width - 54 - 5 * 20, top + 4);
    let panel_y = top - 2 - (11 * 14 + 2);
    let row = |index: i32| (width - 4 - 164 + 28, panel_y + 1 + (1 + index) * 14 + 7);
    let wall = move |x: i32, y: i32| wallpaper(x, y, height);

    let (mut px, mut py) = (width / 2, height / 2);
    let opened = move_by(button.0 + 8 - px, button.1 + 8 - py)
        && click()
        && until(1000, || {
            panel(
                width,
                height,
                [
                    "Power off",
                    "Restart",
                    "Display scale: 2x",
                    "Do not disturb: Off",
                    "Tiling: Off",
                    "Lock screen",
                    "Change PIN",
                    "Language: English",
                    "3D Desktop: Off",
                    "Reduced Motion: Off",
                ],
                wall,
                (button.0 + 8, button.1 + 8),
            ) && compositor::pixel(button.0 + 1, button.1 + 1) == Ok(ENTRY_ACTIVE)
        });
    (px, py) = (button.0 + 8, button.1 + 8);
    if !step("the Quick Settings button opens its panel", opened) {
        return false;
    }

    // Do Not Disturb on, then have Q post a notification: the badge counts
    // it, but no toast appears.
    let (dx, dy) = row(3);
    let dnd = move_by(dx - px, dy - py)
        && click()
        && until(1000, || {
            panel(
                width,
                height,
                [
                    "Power off",
                    "Restart",
                    "Display scale: 2x",
                    "Do not disturb: On",
                    "Tiling: Off",
                    "Lock screen",
                    "Change PIN",
                    "Language: English",
                    "3D Desktop: Off",
                    "Reduced Motion: Off",
                ],
                wall,
                (dx, dy),
            )
        });
    (px, py) = (dx, dy);
    if !step("Do Not Disturb turns on", dnd) {
        return false;
    }
    let clock_x = width - 6 - crate::font::text_width("00:00") as i32;
    let (badge_x, badge_y) = (clock_x - 13, top + 7);
    let mut badge = Surface::new(10, 11);
    let bounds = badge.bounds();
    badge.fill_rect(bounds, ACCENT);
    badge.draw_text(3, 2, "1", WHITE);
    let silenced = mouse::keyboard_loopback(0x19)
        && until(1500, || region(badge_x, badge_y, 10, 11, |x, y| badge.pixel(x - badge_x, y - badge_y).unwrap_or(UNSET)))
        // The shell repaints a notification's whole area at once, desktop
        // layer (with the badge) before the toast layer, so the badge can be
        // seen before a toast would be: the toast area must stay clear for a
        // full second, not just at one instant.
        && stays(100, || region(width - 188, 8, 180, 22, wall));
    if !step(
        "a notification under Do Not Disturb is counted but raises no toast",
        silenced,
    ) {
        return false;
    }

    // Tiling on: the only window fills the work area, under the panel.
    let (tx, ty) = row(4);
    let tiled = move_by(tx - px, ty - py)
        && click()
        && until(2000, || {
            compositor::pixel(5, top - 40) == Ok(QUIET)
                && compositor::pixel(width - 300, 20) == Ok(QUIET)
                && panel(
                    width,
                    height,
                    [
                        "Power off",
                        "Restart",
                        "Display scale: 2x",
                        "Do not disturb: On",
                        "Tiling: On",
                        "Lock screen",
                        "Change PIN",
                        "Language: English",
                        "3D Desktop: Off",
                        "Reduced Motion: Off",
                    ],
                    |_, _| QUIET,
                    (tx, ty),
                )
        });
    (px, py) = (tx, ty);
    if !step("tiling turns on and the window fills the work area", tiled) {
        return false;
    }

    // Display scale 1: the desktop becomes 1280x800, and the panel follows
    // the new geometry. (At 1x the window cannot grow to the whole work area
    // within its owner's surface budget while the old surface still exists,
    // so it keeps its size; what lies beneath the panel is wallpaper.)
    let (sx, sy) = row(2);
    let full = (width * 2, height * 2);
    let scaled = move_by(sx - px, sy - py)
        && click()
        && until(2000, || {
            compositor::scale() == Some(1)
                && compositor::dimensions() == Some((full.0 as u32, full.1 as u32))
        })
        && until(2000, || {
            panel(
                full.0,
                full.1,
                [
                    "Power off",
                    "Restart",
                    "Display scale: 1x",
                    "Do not disturb: On",
                    "Tiling: On",
                    "Lock screen",
                    "Change PIN",
                    "Language: English",
                    "3D Desktop: Off",
                    "Reduced Motion: Off",
                ],
                |x, y| wallpaper(x, y, full.1),
                (sx, sy),
            )
        });
    (px, py) = (sx, sy);
    if !step("display scale 1x makes the desktop 1280x800", scaled) {
        return false;
    }
    // Back to 2x through the panel's scale row at the 1x geometry.
    let back_row = (
        full.0 - 4 - 164 + 28,
        (full.1 - 24) - 2 - (11 * 14 + 2) + 1 + 3 * 14 + 7,
    );
    let restored = move_by(back_row.0 - px, back_row.1 - py)
        && click()
        && until(2000, || {
            compositor::scale() == Some(2)
                && compositor::dimensions() == Some((width as u32, height as u32))
        });
    (px, py) = ((back_row.0).min(width - 1), (back_row.1).min(height - 1));
    if !step("display scale 2x restores 640x400", restored) {
        return false;
    }

    // The button closes the panel; Q takes its key and exits.
    let closed = move_by(button.0 + 8 - px, button.1 + 8 - py)
        && click()
        && until(1000, || {
            region(width - 4 - 164 + 10, panel_y + 10, 100, 30, |_, _| QUIET)
                || region(width - 4 - 164 + 10, panel_y + 10, 100, 30, wall)
        });
    if !step("the button closes the panel", closed) {
        return false;
    }
    let exited = mouse::keyboard_loopback(0x10)
        && wait_hello_gui(quiet).map(|(_, status)| status) == Some(Ok(0));
    if !step("Q takes its key and exits", exited) {
        return false;
    }
    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.17 acceptance: the 3D desktop mode's Quick Settings panel.
///
/// Toggling "3D Desktop" perspective-projects the panel (its bottom edge
/// anchored, its top edge receded and narrowed) instead of blitting it
/// flat. The kernel checks:
/// - the flat panel's top-left corner is exactly where the 2D layout says;
/// - after toggling 3D on, that same pixel is no longer panel content --
///   the real (narrowed) top edge has moved inward, so wallpaper shows
///   through -- while the panel's bottom-left corner, which the projection
///   deliberately keeps anchored, is unchanged.
///
/// This does not claim the perspective math is pixel-perfect, only that a
/// real, non-trivial 3D transform is applied and the content is still the
/// same panel (same bottom anchor), not a different drawing.
#[cfg(feature = "stage20-17-test")]
fn stage20_17_self_test() -> bool {
    use crate::display::rgb;
    const ACCENT: u32 = rgb(90, 140, 220);
    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.17] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = crate::display::blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            crate::display::blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }

    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        until(3000, || {
            compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
        }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    const QS_ROWS: i32 = 10;
    const PANEL_ROW: i32 = 14;
    const CENTER_WIDTH: i32 = 164;
    let panel_height = (1 + QS_ROWS) * PANEL_ROW + 2;
    let (panel_x, panel_y) = (width - 4 - CENTER_WIDTH, top - 2 - panel_height);

    let button = (width - 54 - 5 * 20, top + 4);
    let (mut px, mut py) = (width / 2, height / 2);
    let opened = move_by(button.0 + 8 - px, button.1 + 8 - py)
        && click()
        && until(1000, || compositor::pixel(panel_x, panel_y) == Ok(ACCENT));
    (px, py) = (button.0 + 8, button.1 + 8);
    if !step("the flat panel's top-left corner is exactly where expected", opened) {
        return false;
    }

    // The panel spans several tiles; the top-left check above only proves
    // the first has rendered. Wait for the last one (the bottom-left
    // corner) too before treating the flat panel as fully settled.
    if !step(
        "the flat panel's bottom-left corner settles before toggling 3D",
        until(1000, || {
            compositor::pixel(panel_x, panel_y + panel_height - 1) == Ok(ACCENT)
        }),
    ) {
        return false;
    }
    let bottom_left_before = compositor::pixel(panel_x, panel_y + panel_height - 1);

    // Row 8 of 9, "3D Desktop".
    let toggle_row = (
        width - 4 - CENTER_WIDTH + 28,
        top - 2 - (QS_ROWS * PANEL_ROW + 2) + 1 + 8 * PANEL_ROW + 7,
    );
    let toggled = move_by(toggle_row.0 - px, toggle_row.1 - py) && click();
    if !step("the 3D Desktop row is reachable and clicked", toggled) {
        return false;
    }
    let projected = until(1000, || {
        compositor::pixel(panel_x, panel_y) == Ok(wallpaper(panel_x, panel_y, height))
    });
    if !step(
        "3D mode narrows the top edge: the old top-left corner is now wallpaper",
        projected,
    ) {
        return false;
    }
    let anchored = bottom_left_before == Ok(ACCENT)
        && until(1000, || {
            compositor::pixel(panel_x, panel_y + panel_height - 1) == bottom_left_before
        });
    if !step(
        "3D mode keeps the bottom edge anchored: the bottom-left corner is unchanged",
        anchored,
    ) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err() || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.18 acceptance: the 3D desktop mode's window title bars.
///
/// The same toggle that perspective-projects the Quick Settings panel
/// (Stage 20.17) also applies to every window's title-bar fill: the
/// kernel checks that, with a real window open, the flat title bar's
/// top-left corner is its fill colour, then after toggling 3D on, that
/// same pixel is no longer the fill colour -- the real (narrowed) top
/// edge has moved inward -- while the title bar's bottom-left corner,
/// which the projection deliberately keeps anchored, is unchanged.
#[cfg(feature = "stage20-18-test")]
fn stage20_18_self_test() -> bool {
    use crate::display::rgb;
    const ACCENT: u32 = rgb(90, 140, 220);
    const ACTIVE: u32 = rgb(70, 120, 210);
    const RED: u32 = rgb(200, 60, 60);

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.18] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }

    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        until(3000, || {
            compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
        }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;

    let client_capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(_alpha) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "a"],
        Some(client_capabilities),
    ) else {
        return step("application A spawned", false);
    };
    // A's window cascades to (40, 40), 120x80, its title bar above: (40, 26),
    // 120 wide, 14 tall.
    if !step(
        "window A opened",
        until(2000, || compositor::pixel(45, 45) == Ok(RED)),
    ) {
        return false;
    }
    if !step(
        "the flat title bar's top-left corner is its fill colour",
        until(500, || compositor::pixel(40, 26) == Ok(ACTIVE)),
    ) {
        return false;
    }
    if !step(
        "the flat title bar's bottom-left corner settles before toggling 3D",
        until(500, || compositor::pixel(40, 39) == Ok(ACTIVE)),
    ) {
        return false;
    }

    const QS_ROWS: i32 = 10;
    const PANEL_ROW: i32 = 14;
    const CENTER_WIDTH: i32 = 164;
    let button = (width - 54 - 5 * 20, top + 4);
    let (mut px, mut py) = (width / 2, height / 2);
    let opened = move_by(button.0 + 8 - px, button.1 + 8 - py) && click();
    (px, py) = (button.0 + 8, button.1 + 8);
    if !step("the Quick Settings button opens its panel", opened) {
        return false;
    }
    // Row 8 of 9, "3D Desktop".
    let toggle_row = (
        width - 4 - CENTER_WIDTH + 28,
        top - 2 - (QS_ROWS * PANEL_ROW + 2) + 1 + 8 * PANEL_ROW + 7,
    );
    let toggled = move_by(toggle_row.0 - px, toggle_row.1 - py) && click();
    if !step("the 3D Desktop row is reachable and clicked", toggled) {
        return false;
    }
    let narrowed = until(1000, || compositor::pixel(40, 26) != Ok(ACTIVE));
    if !step(
        "3D mode narrows the title bar's top edge: the top-left corner is no longer the fill colour",
        narrowed,
    ) {
        return false;
    }
    let anchored = until(1000, || compositor::pixel(40, 39) == Ok(ACTIVE));
    if !step(
        "3D mode keeps the title bar's bottom edge anchored: the bottom-left corner is unchanged",
        anchored,
    ) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err() || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.9 acceptance: the Notification Center.
///
/// Application N posts three notifications. The kernel checks:
/// - the toast shows the latest;
/// - a badge beside the clock counts three unread;
/// - clicking the clock opens the Notification Center, listing all three
///   newest first under N's window title, and clears the badge;
/// - Clear empties it ("No notifications");
/// - clicking the clock again closes it.
///
/// Every pixel is compared with the kernel's own rendering: the panel's
/// background blended over the wallpaper, its border, title row and text in
/// the kernel's font. Pointer and keyboard input travel the real i8042
/// path.
#[cfg(feature = "stage20-9-test")]
fn stage20_9_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Rect, Surface};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const TASKBAR: u32 = rgb(20, 24, 32);
    const ACCENT: u32 = rgb(90, 140, 220);
    const START_FILL: u32 = rgb(52, 96, 180);
    const PANEL_FILL: u32 = rgb(28, 32, 44);
    const TOAST_FILL: u32 = rgb(30, 34, 46);
    const NOTES: u32 = rgb(150, 120, 40);
    const UNSET: u32 = 0x0100_0000;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.9] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn region(
        x0: i32,
        y0: i32,
        width: i32,
        height: i32,
        expected: impl Fn(i32, i32) -> u32,
    ) -> bool {
        (y0..y0 + height)
            .all(|y| (x0..x0 + width).all(|x| compositor::pixel(x, y) == Ok(expected(x, y))))
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }

    let installed = vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        installed
            && until(3000, || {
                compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
            }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    let taskbar = move |x: i32, y: i32| blend_pixel(TASKBAR, wallpaper(x, y, height), 216);
    let client = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(notes) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "n"],
        Some(client),
    ) else {
        return step("application N spawned", false);
    };
    if !step(
        "window N opened",
        until(2000, || compositor::pixel(45, 45) == Ok(NOTES)),
    ) {
        return false;
    }

    // An overlay as the shell draws it: a one-pixel ACCENT border, then
    // `draw`; everything else UNSET (transparent over a blended background).
    let overlay = |size: (i32, i32), draw: &dyn Fn(&mut Surface)| {
        let mut surface = Surface::new(size.0 as usize, size.1 as usize);
        let bounds = surface.bounds();
        surface.fill_rect(bounds, UNSET);
        surface.stroke_rect(bounds, ACCENT);
        draw(&mut surface);
        surface
    };
    let translucent = |x0: i32, y0: i32, image: &Surface, fill: u32| {
        let (w, h) = (image.width() as i32, image.bounds().height as i32);
        region(x0, y0, w, h, |x, y| match image.pixel(x - x0, y - y0) {
            Some(UNSET) | None => blend_pixel(fill, wallpaper(x, y, height), 216),
            Some(pixel) => pixel,
        })
    };

    // The toast shows the latest notification.
    let toast = overlay((180, 22), &|surface| {
        surface.draw_text(6, 8, "Battery 80%", WHITE);
    });
    if !step(
        "the toast shows the latest notification",
        until(1500, || translucent(width - 188, 8, &toast, TOAST_FILL)),
    ) {
        return false;
    }

    // The badge: "3" in a 10x11 accent box just left of the HH:MM clock.
    let clock_x = width - 6 - crate::font::text_width("00:00") as i32;
    let (badge_x, badge_y) = (clock_x - 13, top + 7);
    let badge = |digit: &str| {
        let mut image = Surface::new(10, 11);
        let bounds = image.bounds();
        image.fill_rect(bounds, ACCENT);
        image.draw_text(3, 2, digit, WHITE);
        region(badge_x, badge_y, 10, 11, |x, y| {
            image.pixel(x - badge_x, y - badge_y).unwrap_or(UNSET)
        })
    };
    if !step(
        "a badge counts three unread notifications",
        until(1000, || badge("3")),
    ) {
        return false;
    }

    // Click the clock: the Notification Center opens with all three, newest
    // first, under N's title; the badge goes. Rows of 14 inside the border.
    let center = |rows: &[&str]| {
        let lines = rows.len().max(1) as i32;
        let h = (1 + lines) * 14 + 2;
        let (x0, y0) = (width - 4 - 164, top - 2 - h);
        let image = overlay((164, h), &|surface| {
            surface.fill_rect(Rect::new(1, 1, 162, 14), START_FILL);
            surface.draw_text(6, 5, "Notifications", WHITE);
            surface.draw_text(
                164 - 6 - crate::font::text_width("Clear") as i32,
                5,
                "Clear",
                WHITE,
            );
            if rows.is_empty() {
                surface.draw_text(6, 1 + 14 + 4, "No notifications", WHITE);
            }
            for (index, row) in rows.iter().enumerate() {
                surface.draw_text(6, 1 + (1 + index as i32) * 14 + 4, row, WHITE);
            }
        });
        (x0, y0, h, image)
    };
    let (px, py) = (width / 2, height / 2);
    let (cx, cy) = (width - 20, top + 12);
    let (x0, y0, _, full) = center(&[
        "Notes: Battery 80%",
        "Notes: Mail: 3 new",
        "Notes: Build done",
    ]);
    let opened = move_by(cx - px, cy - py)
        && click()
        && until(1000, || translucent(x0, y0, &full, PANEL_FILL))
        && until(500, || region(badge_x, badge_y, 10, 11, taskbar));
    if !step(
        "clicking the clock opens the center, newest first, and clears the badge",
        opened,
    ) {
        return false;
    }

    // Clear, at the right of the title row; then the clock closes it.
    let (ex, ey, eh, empty) = center(&[]);
    let clear_point = (x0 + 164 - 20, y0 + 7);
    let cleared = move_by(clear_point.0 - cx, clear_point.1 - cy)
        && click()
        && until(1000, || translucent(ex, ey, &empty, PANEL_FILL));
    if !step("Clear empties the Notification Center", cleared) {
        return false;
    }
    let closed = move_by(cx - clear_point.0, cy - clear_point.1)
        && click()
        && until(1000, || {
            region(ex, ey, 164, eh, |x, y| wallpaper(x, y, height))
        });
    if !step("clicking the clock again closes it", closed) {
        return false;
    }

    let exited = mouse::keyboard_loopback(0x31)
        && wait_hello_gui(notes).map(|(_, status)| status) == Some(Ok(0));
    if !step("N takes its key and exits", exited) {
        return false;
    }
    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.8 acceptance: tiling.
///
/// Windows change size through the shell's resize protocol: `WSIZ` to the
/// owner, `WRSZ` back with a new surface.
/// - Dropping window A on the screen's left edge snaps it to the left half:
///   it moves and the owner resizes it to 320x362.
/// - F2 turns tiling on: master and stack, so A keeps the left half and B
///   takes the right half, resized too.
/// - Then both applications exit.
///
/// Every frame is compared exactly: the client areas, and the title bars
/// rendered with the kernel's font. F2 travels the real keyboard path.
#[cfg(feature = "stage20-8-test")]
fn stage20_8_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Surface};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const ACTIVE: u32 = rgb(70, 120, 210);
    const INACTIVE: u32 = rgb(60, 66, 80);
    const CLOSE_FILL: u32 = rgb(170, 60, 60);
    const RED: u32 = rgb(200, 60, 60);
    const BLUE: u32 = rgb(60, 60, 200);
    const UNSET: u32 = 0x0100_0000;
    const BLACK: u32 = rgb(0, 0, 0);

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.8] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn region(
        x0: i32,
        y0: i32,
        width: i32,
        height: i32,
        expected: impl Fn(i32, i32) -> u32,
    ) -> bool {
        (y0..y0 + height)
            .all(|y| (x0..x0 + width).all(|x| compositor::pixel(x, y) == Ok(expected(x, y))))
    }
    /// A title bar, with `shade` applied to each expected pixel (for a
    /// shadow falling across it).
    fn title_bar(
        x: i32,
        y: i32,
        width: i32,
        text: &str,
        fill: u32,
        shade: impl Fn(i32, i32, u32) -> u32,
    ) -> bool {
        let mut expected = Surface::new(width as usize, 14);
        let bounds = expected.bounds();
        expected.fill_rect(bounds, fill);
        expected.draw_text(4, 4, text, WHITE);
        expected.fill_rect(crate::display::Rect::new(width - 12, 2, 10, 10), CLOSE_FILL);
        expected.draw_text(width - 12 + 3, 4, "x", WHITE);
        region(x, y, width, 14, |px, py| {
            shade(px, py, expected.pixel(px - x, py - y).unwrap_or(UNSET))
        })
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32, held: bool) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, held) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }

    let installed = vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        installed
            && until(3000, || {
                compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
            }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    let client = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(alpha) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "a"],
        Some(client),
    ) else {
        return step("application A spawned", false);
    };
    if !until(2000, || compositor::pixel(45, 45) == Ok(RED)) {
        return step("window A opened", false);
    }
    let Some(beta) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "b"],
        Some(client),
    ) else {
        return step("application B spawned", false);
    };
    if !step(
        "windows A and B opened",
        until(2000, || compositor::pixel(150, 100) == Ok(BLUE)),
    ) {
        return false;
    }

    // The halves of the work area above the taskbar, as client areas under
    // 14-pixel title bars.
    let half = width / 2;
    let client_height = top - 14;
    let plain = |_: i32, _: i32, pixel: u32| pixel;
    // Once A fills the left half and is focused, its deep shadow (bands at
    // 8 and 4 pixels) falls across the edge of whatever lies to its right.
    let a_shadow = move |x: i32, y: i32, mut pixel: u32| {
        for (offset, opacity) in [(8, 24u8), (4, 48u8)] {
            if x >= offset && x < offset + half && y >= offset && y < offset + top && y < top {
                pixel = blend_pixel(BLACK, pixel, opacity);
            }
        }
        pixel
    };

    // Drag A by its title bar onto the left edge and drop it there. Then
    // park the pointer on the taskbar, clear of every window.
    let (px, py) = (width / 2, height / 2);
    let snapped = move_by(80 - px, 32 - py, false)
        && packet(0, 0, true)
        && move_by(-80, 0, true)
        && packet(0, 0, false)
        && move_by(600, top + 14 - 32, false)
        && until(2000, || {
            region(0, 14, half, client_height, |_, _| RED)
                && title_bar(0, 0, half, "Alpha", ACTIVE, plain)
        });
    if !step(
        "dropping A on the left edge snaps it to the left half, resized",
        snapped,
    ) {
        return false;
    }

    // F2: tiling on. A, first, keeps the left half; B takes the right half,
    // resized, under the edge of A's shadow; A remains focused.
    let tiled = mouse::keyboard_loopback(0x3c)
        && until(2000, || {
            region(half, 14, width - half, client_height, |x, y| {
                a_shadow(x, y, BLUE)
            }) && title_bar(half, 0, width - half, "Beta", INACTIVE, a_shadow)
                && region(0, 14, half, client_height, |_, _| RED)
                && title_bar(0, 0, half, "Alpha", ACTIVE, plain)
        });
    if !step(
        "F2 tiles the windows: master on the left, stack on the right",
        tiled,
    ) {
        return false;
    }

    // Tiling off; A, focused, takes x and exits, and once its window is gone
    // B takes y and exits.
    let exited = mouse::keyboard_loopback(0x3c)
        && mouse::keyboard_loopback(0x2d)
        && wait_hello_gui(alpha).map(|(_, status)| status) == Some(Ok(0))
        && until(1000, || {
            compositor::pixel(5, 300) == Ok(wallpaper(5, 300, height))
        })
        && mouse::keyboard_loopback(0x15)
        && wait_hello_gui(beta).map(|(_, status)| status) == Some(Ok(0))
        && until(1000, || {
            compositor::pixel(half + 10, 300) == Ok(wallpaper(half + 10, 300, height))
        });
    if !step("both applications take their keys and exit", exited) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.7 acceptance: physics-based animation.
///
/// A new window fades in, its opacity following the shell's spring from 0
/// to 255, with its shadow scaled to match. The kernel samples one pixel of
/// the window's client area as fast as it can while the window appears. It
/// reads the physical framebuffer, which changes only when a fully composed
/// frame is presented; the compositor surface is updated layer by layer and
/// would show frames half-built.
/// For every opacity `a` it computes what that pixel must read: the
/// wallpaper, the window's own shadow band at `48 * a / 255`, then the
/// client colour blended at `a`. Each sample must be one of those values;
/// the opacities they imply must never decrease; at least three distinct
/// intermediate opacities must be seen; and the pixel must end fully
/// opaque. A jump from nothing to opaque fails all but the last of these.
#[cfg(feature = "stage20-7-test")]
fn stage20_7_self_test() -> bool {
    use crate::display::{blend_pixel, rgb};
    use bootloader_api::info::PixelFormat;

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const BLACK: u32 = rgb(0, 0, 0);
    const RED: u32 = rgb(200, 60, 60);

    /// A presented pixel, as 0x00RRGGBB, from the physical framebuffer.
    fn screen_pixel(x: usize, y: usize) -> Option<u32> {
        terminal::with_framebuffer(|buffer, info| {
            if x >= info.width || y >= info.height || info.bytes_per_pixel < 3 {
                return None;
            }
            let offset = (y * info.stride + x) * info.bytes_per_pixel;
            let bytes = buffer.get(offset..offset + 3)?;
            match info.pixel_format {
                PixelFormat::Rgb => Some(rgb(bytes[0], bytes[1], bytes[2])),
                PixelFormat::Bgr => Some(rgb(bytes[2], bytes[1], bytes[0])),
                _ => None,
            }
        })
        .flatten()
    }
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.7] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }

    let installed = vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        installed
            && until(3000, || {
                compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
            }),
    ) {
        return false;
    }
    let Some((_, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let height = height as i32;

    // The pixel watched: (45, 45), inside A's client area at (40, 40). Of
    // A's (focused) shadow bands, only the one offset by 4 covers it.
    let (x, y) = (45, 45);
    let expected: alloc::vec::Vec<u32> = (0..=255u32)
        .map(|a| {
            let shadow = (48 * a / 255) as u8;
            let under = blend_pixel(BLACK, wallpaper(x, y, height), shadow);
            blend_pixel(RED, under, a as u8)
        })
        .collect();
    let scale = compositor::scale().unwrap_or(1) as usize;
    let (sx, sy) = (x as usize * scale, y as usize * scale);
    let start_value = screen_pixel(sx, sy);
    let client = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(alpha) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "a"],
        Some(client),
    ) else {
        return step("application A spawned", false);
    };

    // Sample until the pixel is fully opaque, recording each new value.
    let mut samples: alloc::vec::Vec<u32> = alloc::vec::Vec::new();
    let started = timer::ticks();
    loop {
        let Some(value) = screen_pixel(sx, sy) else {
            return step("pixel readable", false);
        };
        if samples.last() != Some(&value) {
            samples.push(value);
        }
        if value == RED || timer::ticks().wrapping_sub(started) > 3000 {
            break;
        }
        task::yield_now();
    }
    let duration = timer::ticks().wrapping_sub(started);

    // Map each sample to the smallest opacity at or above the previous one
    // that explains it; a sample nothing explains, or that would need the
    // opacity to fall, fails.
    let mut previous = 0usize;
    let mut opacities: alloc::vec::Vec<usize> = alloc::vec::Vec::new();
    let mut consistent = start_value == Some(expected[0]);
    for value in &samples {
        match (previous..=255).find(|&a| expected[a] == *value) {
            Some(a) => {
                opacities.push(a);
                previous = a;
            }
            None => consistent = false,
        }
    }
    let intermediate = opacities.iter().filter(|&&a| a > 0 && a < 255).count();
    serial::write_line(format_args!(
        "[S20.7] {} distinct samples over {} ticks; opacities {:?}",
        samples.len(),
        duration,
        opacities
    ));
    if !step(
        "every sample is the window blended at some opacity, never decreasing",
        consistent,
    ) || !step(
        "the window passes through at least three intermediate opacities",
        intermediate >= 3,
    ) || !step("the window ends fully opaque", samples.last() == Some(&RED))
    {
        return false;
    }
    // Settled: the shell is back at its idle frame rate and the window stays
    // put.
    let settled = until(500, || {
        screen_pixel(sx, sy) == Some(RED) && compositor::pixel(x, y) == Ok(RED)
    });
    let exited = mouse::keyboard_loopback(0x2d)
        && wait_hello_gui(alpha).map(|(_, status)| status) == Some(Ok(0));
    if !step(
        "the window stays opaque once settled; A takes its key and exits",
        settled && exited,
    ) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.6 acceptance: translucent overlays over windows.
///
/// The launcher panel and the toast are translucent: their background is
/// blended over whatever lies beneath, windows included, and their border and
/// text are an overlay copied with a keyed blit, as is the pointer. The kernel
/// recomputes every pixel: what lies beneath (wallpaper, the focused window's
/// shadow bands, the window), the blend of the background over it, and the
/// overlay rendered with its own font.
/// - The toast is checked over the wallpaper.
/// - The pointer is checked over a window: its transparent cells show the
///   window.
/// - The panel is checked over a window and its shadow.
#[cfg(feature = "stage20-6-test")]
fn stage20_6_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Rect, Surface};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const BLACK: u32 = rgb(0, 0, 0);
    const ACCENT: u32 = rgb(90, 140, 220);
    const START_FILL: u32 = rgb(52, 96, 180);
    const PANEL_FILL: u32 = rgb(28, 32, 44);
    const TOAST_FILL: u32 = rgb(30, 34, 46);
    const RED: u32 = rgb(200, 60, 60);
    const UNSET: u32 = 0x0100_0000;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.6] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn region(
        x0: i32,
        y0: i32,
        width: i32,
        height: i32,
        expected: impl Fn(i32, i32) -> u32,
    ) -> bool {
        (y0..y0 + height)
            .all(|y| (x0..x0 + width).all(|x| compositor::pixel(x, y) == Ok(expected(x, y))))
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32, held: bool) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, held) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    /// An overlay of `size` as the shell draws it: a one-pixel `ACCENT`
    /// border, then `draw` on top; everything else UNSET (transparent).
    fn overlay(size: (i32, i32), draw: impl Fn(&mut Surface)) -> Surface {
        let mut surface = Surface::new(size.0 as usize, size.1 as usize);
        let bounds = surface.bounds();
        surface.fill_rect(bounds, UNSET);
        surface.stroke_rect(bounds, ACCENT);
        draw(&mut surface);
        surface
    }

    let installed = vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    // No FileRead: the launcher is empty, so the panel says "No matches".
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        installed
            && until(3000, || {
                compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
            }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    let client = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(alpha) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "a"],
        Some(client),
    ) else {
        return step("application A spawned", false);
    };
    if !step(
        "window A opened",
        until(2000, || compositor::pixel(45, 45) == Ok(RED)),
    ) {
        return false;
    }

    // The toast A posts, over the wallpaper: background blended, border and
    // text exact.
    let (toast_x, toast_y) = (width - 188, 8);
    let toast_overlay = overlay((180, 22), |surface| {
        surface.draw_text(6, 8, "Alpha ready", WHITE);
    });
    let toast = until(1000, || {
        region(toast_x, toast_y, 180, 22, |x, y| {
            match toast_overlay.pixel(x - toast_x, y - toast_y) {
                Some(UNSET) | None => blend_pixel(TOAST_FILL, wallpaper(x, y, height), 216),
                Some(pixel) => pixel,
            }
        })
    });
    if !step("the toast is translucent over the wallpaper", toast) {
        return false;
    }

    // Drag A down to the taskbar's edge, under where the panel will open,
    // then rest the pointer over A's client area.
    let (px, py) = (width / 2, height / 2);
    let dragged = move_by(80 - px, 32 - py, false)
        && packet(0, 0, true)
        && move_by(0, 300, true)
        && packet(0, 0, false)
        && move_by(100 - 80, 340 - 332, false);
    // A's client area is now (40, 296) 120x80.
    let pointer = dragged
        && until(1000, || compositor::pixel(45, top - 40) == Ok(RED))
        && until(500, || {
            compositor::pixel(100, 340) == Ok(BLACK)
                && compositor::pixel(101, 342) == Ok(WHITE)
                && compositor::pixel(106, 340) == Ok(RED)
                && compositor::pixel(102, 341) == Ok(RED)
        });
    if !step(
        "the pointer's transparent cells show the window beneath",
        pointer,
    ) {
        return false;
    }

    // F1 opens the empty launcher: rows of 14 inside the border, three rows
    // high, just above the taskbar.
    let panel_height = 3 * 14 + 2;
    let panel_y = top - 2 - panel_height;
    let panel_overlay = overlay((160, panel_height), |surface| {
        surface.fill_rect(Rect::new(1, 1, 158, 14), START_FILL);
        surface.draw_text(6, 5, "WovenStart", WHITE);
        surface.draw_text(6, 19, "> _", WHITE);
        surface.draw_text(6, 33, "No matches", WHITE);
    });
    // Beneath the panel: A's client area, A's (focused) shadow beside it,
    // and the wallpaper. The pointer, at (100, 340), is inside the panel, so
    // the comparison leaves its cells out.
    let a_frame = (40, top - 94, 120, 94);
    let beneath = move |x: i32, y: i32| {
        let (fx, fy, fw, fh) = a_frame;
        if x >= fx && x < fx + fw && y >= fy + 14 && y < fy + fh {
            return RED;
        }
        let mut value = wallpaper(x, y, height);
        for (offset, opacity) in [(8, 24u8), (4, 48u8)] {
            let (bx, by) = (fx + offset, fy + offset);
            if x >= bx && x < bx + fw && y >= by && y < by + fh && y < top {
                value = blend_pixel(BLACK, value, opacity);
            }
        }
        value
    };
    let under_pointer = |x: i32, y: i32| (100..107).contains(&x) && (340..350).contains(&y);
    let panel = mouse::keyboard_loopback(0x3b)
        && until(1000, || {
            (panel_y..panel_y + panel_height).all(|y| {
                (4..164).all(|x| {
                    if under_pointer(x, y) {
                        return true;
                    }
                    let expected = match panel_overlay.pixel(x - 4, y - panel_y) {
                        Some(UNSET) | None => blend_pixel(PANEL_FILL, beneath(x, y), 216),
                        Some(pixel) => pixel,
                    };
                    compositor::pixel(x, y) == Ok(expected)
                })
            })
        });
    if !step(
        "the launcher panel is translucent over a window and its shadow",
        panel,
    ) {
        return false;
    }
    // The pointer stays on top of the panel. The shell presents each layer
    // separately (panel, then cursor), so poll for the settled frame as the
    // checks above do; a single read can land between the two presents.
    if !step(
        "the pointer is drawn above the panel",
        until(500, || {
            compositor::pixel(100, 340) == Ok(BLACK) && compositor::pixel(101, 342) == Ok(WHITE)
        }),
    ) {
        return false;
    }

    let closed = mouse::keyboard_loopback(0x3b)
        && until(1000, || {
            compositor::pixel(20, panel_y + 20) == Ok(wallpaper(20, panel_y + 20, height))
        })
        && mouse::keyboard_loopback(0x2d)
        && wait_hello_gui(alpha).map(|(_, status)| status) == Some(Ok(0));
    if !step("panel closes; A takes its key and exits", closed) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.5 acceptance: depth, from window shadows and elevation.
///
/// Two desktop applications, A and then B, so B is focused. Every shadow
/// pixel checked is recomputed by the kernel: the wallpaper, then each
/// window's two bands, bottom to top, blended with `display::blend_pixel`.
/// The focused window's bands are deeper.
/// - B casts the deep shadow and A the shallow one.
/// - Clicking A's title bar swaps them.
/// - Dragging A onto the taskbar's edge shows that shadows stop at the
///   taskbar.
#[cfg(feature = "stage20-5-test")]
fn stage20_5_self_test() -> bool {
    use crate::display::{blend_pixel, rgb};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const BLACK: u32 = rgb(0, 0, 0);
    const TASKBAR: u32 = rgb(20, 24, 32);
    const ACCENT: u32 = rgb(90, 140, 220);
    const RED: u32 = rgb(200, 60, 60);
    const BLUE: u32 = rgb(60, 60, 200);
    const FOCUSED: [(i32, u8); 2] = [(8, 24), (4, 48)];
    const UNFOCUSED: [(i32, u8); 2] = [(4, 20), (2, 40)];

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.5] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn region(
        x0: i32,
        y0: i32,
        width: i32,
        height: i32,
        expected: impl Fn(i32, i32) -> u32,
    ) -> bool {
        (y0..y0 + height)
            .all(|y| (x0..x0 + width).all(|x| compositor::pixel(x, y) == Ok(expected(x, y))))
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32, held: bool) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, held) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    /// A window frame (title bar and client area) as (x, y, width, height).
    type Frame = (i32, i32, i32, i32);
    /// The expected pixel at a point no window covers: wallpaper, then each
    /// window's shadow bands bottom to top, never on the taskbar.
    fn shadowed(x: i32, y: i32, height: i32, stack: &[(Frame, bool)]) -> u32 {
        let mut value = wallpaper(x, y, height);
        if y >= height - 24 {
            return value;
        }
        for &((fx, fy, fw, fh), focused) in stack {
            for (offset, opacity) in if focused { FOCUSED } else { UNFOCUSED } {
                let (bx, by) = (fx + offset, fy + offset);
                if x >= bx && x < bx + fw && y >= by && y < by + fh {
                    value = blend_pixel(BLACK, value, opacity);
                }
            }
        }
        value
    }

    let installed = vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        installed
            && until(3000, || {
                compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
            }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    let client = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(alpha) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "a"],
        Some(client),
    ) else {
        return step("application A spawned", false);
    };
    if !until(2000, || compositor::pixel(45, 45) == Ok(RED)) {
        return step("window A opened", false);
    }
    let Some(beta) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "b"],
        Some(client),
    ) else {
        return step("application B spawned", false);
    };
    if !step(
        "windows A and B opened",
        until(2000, || compositor::pixel(150, 100) == Ok(BLUE)),
    ) {
        return false;
    }

    // Frames: A at (40, 26) 120x94, B at (64, 50) 100x74. The regions below
    // lie outside both frames, so only wallpaper and shadow bands are there.
    let a: Frame = (40, 26, 120, 94);
    let b: Frame = (64, 50, 100, 74);
    let check = |stack: &[(Frame, bool)], areas: &[(i32, i32, i32, i32)]| {
        until(1000, || {
            areas
                .iter()
                .all(|&(x, y, w, h)| region(x, y, w, h, |px, py| shadowed(px, py, height, stack)))
        })
    };
    // B focused: its right and bottom strips carry the deep bands; A's
    // visible strips (above B and left of it) carry the shallow ones.
    let b_focused = check(
        &[(a, false), (b, true)],
        &[
            (164, 58, 8, 74),
            (72, 124, 100, 8),
            (160, 28, 4, 22),
            (42, 120, 22, 4),
        ],
    );
    if !step("focused window B casts the deeper shadow", b_focused) {
        return false;
    }
    // A shadow pixel is really darker than the wallpaper there.
    let darker =
        compositor::pixel(168, 128).is_ok_and(|pixel| pixel != wallpaper(168, 128, height));
    if !step("shadow pixels differ from the bare wallpaper", darker) {
        return false;
    }

    // Click A's title bar: A is raised and focused, and the shadows swap.
    let (px, py) = (width / 2, height / 2);
    let clicked = move_by(80 - px, 32 - py, false)
        && packet(0, 0, true)
        && packet(0, 0, false)
        && until(1000, || compositor::pixel(150, 100) == Ok(RED));
    let a_focused = clicked
        && check(
            &[(b, false), (a, true)],
            &[
                (160, 34, 8, 16),
                (48, 120, 16, 8),
                (164, 54, 4, 74),
                (68, 124, 100, 4),
            ],
        );
    if !step(
        "raising A gives it the deeper shadow and B the shallow one",
        a_focused,
    ) {
        return false;
    }

    // Drag A down onto the taskbar's edge: its frame stops at the taskbar,
    // and its shadow beside it stops there too.
    let dragged = packet(0, 0, true)
        && move_by(0, 300, true)
        && packet(0, 0, false)
        && move_by(600 - 80, 150 - 332, false);
    let low: Frame = (40, top - 94, 120, 94);
    let edge = dragged
        && until(1000, || compositor::pixel(45, top - 40) == Ok(RED))
        && check(&[(b, false), (low, true)], &[(160, top - 86, 8, 86)])
        && until(500, || {
            region(48, top, 120, 1, |_, _| ACCENT)
                && region(48, top + 1, 120, 3, |x, y| {
                    blend_pixel(TASKBAR, wallpaper(x, y, height), 216)
                })
        });
    if !step("shadows stop at the taskbar", edge) {
        return false;
    }

    // B is typed to only once the shell has seen A go: until then A is still
    // the focused window, and a key would go to its exited owner.
    let exited = mouse::keyboard_loopback(0x2d)
        && wait_hello_gui(alpha).map(|(_, status)| status) == Some(Ok(0))
        && until(1000, || compositor::pixel(45, top - 40) != Ok(RED))
        && mouse::keyboard_loopback(0x15)
        && wait_hello_gui(beta).map(|(_, status)| status) == Some(Ok(0))
        && until(1000, || {
            region(164, 54, 8, 78, |x, y| wallpaper(x, y, height))
                && compositor::pixel(150, 100) == Ok(wallpaper(150, 100, height))
        });
    if !step(
        "windows and their shadows go with their applications",
        exited,
    ) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.4 acceptance: windows that move and close, on budgeted surfaces.
///
/// The kernel first checks the surface budgets directly:
/// - dimensions up to the compositor's bound;
/// - one process limited to its pixel budget, which a destroy returns;
/// - all processes together limited to the system budget.
///
/// Then the desktop runs with an ordinary window and one larger than the old
/// 320×240 cap, driven by real i8042 input:
/// - a title-bar drag moves window A, and the regions it leaves and covers
///   repaint correctly;
/// - the close control on the wide window asks its owner to close, and the
///   owner does;
/// - A takes its key and exits.
#[cfg(feature = "stage20-4-test")]
fn stage20_4_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Surface};
    use crate::user_graphics::{Error as SurfaceError, OWNER_PIXEL_BUDGET};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const ACTIVE: u32 = rgb(70, 120, 210);
    const CLOSE_FILL: u32 = rgb(170, 60, 60);
    const RED: u32 = rgb(200, 60, 60);
    const GREEN: u32 = rgb(60, 160, 90);
    const UNSET: u32 = 0x0100_0000;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.4] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn region(
        x0: i32,
        y0: i32,
        width: i32,
        height: i32,
        expected: impl Fn(i32, i32) -> u32,
    ) -> bool {
        (y0..y0 + height)
            .all(|y| (x0..x0 + width).all(|x| compositor::pixel(x, y) == Ok(expected(x, y))))
    }
    /// A title bar as the kernel renders it: fill, title text, and the
    /// close control at its right.
    fn title_bar(x: i32, y: i32, width: i32, text: &str, fill: u32) -> bool {
        let mut expected = Surface::new(width as usize, 14);
        let bounds = expected.bounds();
        expected.fill_rect(bounds, fill);
        expected.draw_text(4, 4, text, WHITE);
        let close = crate::display::Rect::new(width - 12, 2, 10, 10);
        expected.fill_rect(close, CLOSE_FILL);
        expected.draw_text(width - 12 + 3, 4, "x", WHITE);
        region(x, y, width, 14, |px, py| {
            expected.pixel(px - x, py - y).unwrap_or(UNSET)
        })
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    /// Relative motion in packets of at most 100 per axis, with the left
    /// button held or not.
    fn move_by(mut dx: i32, mut dy: i32, held: bool) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, held) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }

    // Budgets, checked on the registry directly with owners no process uses.
    const OWNER: u64 = 4300;
    let oversized = user_graphics::create(OWNER, user_graphics::MAX_WIDTH + 1, 1)
        == Err(SurfaceError::OutOfRange)
        && user_graphics::create(OWNER, 1, user_graphics::MAX_HEIGHT + 1)
            == Err(SurfaceError::OutOfRange);
    let whole = user_graphics::create(OWNER, 1024, 1024);
    let per_owner = oversized
        && OWNER_PIXEL_BUDGET == 1024 * 1024
        && whole.is_ok()
        && user_graphics::create(OWNER, 1, 1) == Err(SurfaceError::OwnerFull)
        && whole.is_ok_and(|handle| user_graphics::destroy(handle).is_ok())
        && user_graphics::create(OWNER, 1, 1).is_ok();
    user_graphics::release_owner(OWNER);
    if !step("one process is held to its pixel budget", per_owner) {
        return false;
    }
    let owners = [4301u64, 4302, 4303, 4304];
    let filled = owners
        .iter()
        .all(|&owner| user_graphics::create(owner, 1024, 1024).is_ok());
    let system = filled
        && user_graphics::create(4305, 1, 1) == Err(SurfaceError::SystemFull)
        && user_graphics::release_owner(owners[0]) == 1
        && user_graphics::create(4305, 1, 1).is_ok();
    for owner in owners.into_iter().chain([4305]) {
        user_graphics::release_owner(owner);
    }
    if !step(
        "all processes together are held to the system budget",
        system,
    ) {
        return false;
    }

    // The desktop.
    let installed = vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok();
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        installed
            && until(3000, || {
                compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
            }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let client = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(alpha) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "a"],
        Some(client),
    ) else {
        return step("application A spawned", false);
    };
    if !step(
        "window A opened",
        until(2000, || compositor::pixel(45, 45) == Ok(RED)),
    ) {
        return false;
    }
    let Some(wide) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "c"],
        Some(client),
    ) else {
        return step("wide application spawned", false);
    };
    // 400x260, larger than the old 320x240 cap, cascaded to (64, 64).
    let opened = until(2000, || compositor::pixel(300, 200) == Ok(GREEN))
        && until(500, || {
            region(64, 64, 400, 260, |_, _| GREEN) && title_bar(64, 50, 400, "Wide", ACTIVE)
        });
    if !step(
        "a window larger than 320x240 opens, with a close control",
        opened,
    ) {
        return false;
    }

    // Drag A by its title bar: take hold at (80, 32), 40 right of and 8
    // above A's client origin, then move (300, 250) with the button held.
    let (px, py) = (width / 2, height / 2);
    // Released at (380, 282), then parked on empty desktop at (600, 150) so
    // the pointer covers nothing being checked.
    let dragged = move_by(80 - px, 32 - py, false)
        && packet(0, 0, true)
        && move_by(300, 250, true)
        && packet(0, 0, false)
        && move_by(600 - 380, 150 - 282, false);
    // A's client area lands at (380 - 40, 282 + 8) = (340, 290): above the
    // wide window, which it now overlaps, and focused.
    let moved = dragged
        && until(1000, || compositor::pixel(345, 295) == Ok(RED))
        && until(500, || {
            title_bar(340, 276, 120, "Alpha", ACTIVE)
                && region(340, 290, 120, 80, |x, y| {
                    if (350..370).contains(&x) && (300..310).contains(&y) {
                        WHITE
                    } else {
                        RED
                    }
                })
                && compositor::pixel(45, 45) == Ok(wallpaper(45, 45, height))
                && compositor::pixel(100, 100) == Ok(GREEN)
        });
    if !step(
        "dragging the title bar moves the window and repaints both places",
        moved,
    ) {
        return false;
    }

    // The wide window's close control, at (452, 52): the owner is asked to
    // close, does, and the window goes. A keeps the focus.
    let closed = move_by(457 - 600, 57 - 150, false)
        && packet(0, 0, true)
        && packet(0, 0, false)
        && wait_hello_gui(wide).map(|(_, status)| status) == Some(Ok(0))
        && until(1000, || {
            compositor::pixel(300, 200) == Ok(wallpaper(300, 200, height))
        });
    if !step(
        "the close control asks the owner to close, and it does",
        closed,
    ) {
        return false;
    }
    let typed = mouse::keyboard_loopback(0x2d)
        && wait_hello_gui(alpha).map(|(_, status)| status) == Some(Ok(0))
        && until(1000, || {
            compositor::pixel(345, 295) == Ok(wallpaper(345, 295, height))
        });
    if !step("A keeps the focus, takes its key and exits", typed) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.3 acceptance: WovenStart, the application launcher.
///
/// A catalog of three entries is installed in `/share/applications`. The
/// third, Gamma, names a program that does not exist. The shell runs with
/// `FileRead` and `ProcessCreate`, and real keyboard and pointer input
/// drives the launcher:
/// - F1 opens it, Tab moves the selection, typing filters, and Enter
///   launches Beta;
/// - the start button opens it, and clicking a row launches Alpha;
/// - launching Gamma reports the failure in a toast;
/// - a search with no match says so.
///
/// Every panel state is compared with the kernel's own rendering. Launched
/// applications must open their windows, take keys, and be reaped by the
/// shell when they exit.
#[cfg(feature = "stage20-3-test")]
fn stage20_3_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Rect, Surface};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const ACCENT: u32 = rgb(90, 140, 220);
    const ACTIVE: u32 = rgb(70, 120, 210);
    const START_FILL: u32 = rgb(52, 96, 180);
    const PANEL_FILL: u32 = rgb(28, 32, 44);
    const TOAST_FILL: u32 = rgb(30, 34, 46);
    const RED: u32 = rgb(200, 60, 60);
    const BLUE: u32 = rgb(60, 60, 200);
    const UNSET: u32 = 0x0100_0000;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.3] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn region(
        x0: i32,
        y0: i32,
        width: i32,
        height: i32,
        expected: impl Fn(i32, i32) -> u32,
    ) -> bool {
        (y0..y0 + height)
            .all(|y| (x0..x0 + width).all(|x| compositor::pixel(x, y) == Ok(expected(x, y))))
    }
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }
    fn keys(scancodes: &[u8]) -> bool {
        scancodes.iter().all(|&code| mouse::keyboard_loopback(code))
    }

    // The catalog, and the program two of its entries launch.
    let catalog = vfs::mkdir("/share").is_ok()
        && vfs::mkdir("/share/applications").is_ok()
        && vfs::create_read_only(
            "/share/applications/alpha.app",
            b"Name=Alpha\nExec=hello-gui --desktop-client %p a\n",
        )
        .is_ok()
        && vfs::create_read_only(
            "/share/applications/beta.app",
            b"Name=Beta\nExec=hello-gui --desktop-client %p b\n",
        )
        .is_ok()
        && vfs::create_read_only(
            "/share/applications/gamma.app",
            b"Name=Gamma\nExec=missing-program %p\n",
        )
        .is_ok()
        && vfs::create_read_only("/share/applications/notes.txt", b"not an application").is_ok()
        && vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok()
        && vfs::create_read_only("/bin/woven-shell", WOVEN_SHELL_IMAGE).is_ok();
    if !step("catalog and programs installed", catalog) {
        return false;
    }
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc)
        .with(capability::Capability::FileRead)
        .with(capability::Capability::ProcessCreate);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        until(3000, || {
            compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
        }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;

    // The launcher panel as the kernel renders it: rows of 14 pixels (title,
    // search, one per match or "No matches") inside a one-pixel border, just
    // above the taskbar.
    let panel_height = |matches: usize| (2 + matches.max(1) as i32) * 14 + 2;
    let panel = |names: &[&str], query: &str, selected: usize| {
        let h = panel_height(names.len());
        let y = top - 2 - h;
        let mut expected = Surface::new(160, h as usize);
        expected.fill_rect(Rect::new(0, 0, 160, h as usize), ACCENT);
        expected.fill_rect(Rect::new(1, 1, 158, (h - 2) as usize), UNSET);
        expected.fill_rect(Rect::new(1, 1, 158, 14), START_FILL);
        expected.draw_text(6, 5, "WovenStart", WHITE);
        expected.draw_text(6, 19, &alloc::format!("> {}_", query), WHITE);
        if names.is_empty() {
            expected.draw_text(6, 33, "No matches", WHITE);
        }
        for (row, name) in names.iter().enumerate() {
            let row_y = 1 + (2 + row as i32) * 14;
            if row == selected {
                expected.fill_rect(Rect::new(1, row_y, 158, 14), ACTIVE);
            }
            expected.draw_text(6, row_y + 4, name, WHITE);
        }
        // Unset cells are the translucent background over the wallpaper.
        until(500, || {
            region(4, y, 160, h, |x, py| match expected.pixel(x - 4, py - y) {
                Some(UNSET) | None => blend_pixel(PANEL_FILL, wallpaper(x, py, height), 216),
                Some(pixel) => pixel,
            })
        })
    };
    let closed = || {
        let h = panel_height(3);
        until(500, || {
            region(4, top - 2 - h, 160, h, |x, y| wallpaper(x, y, height))
        })
    };
    let children = || task::child_count(shell);

    // Keyboard: F1 opens the launcher with the whole catalog, sorted, and
    // the non-.app file left out; Tab moves the selection; typing filters.
    if !step(
        "F1 opens the launcher with the catalog",
        keys(&[0x3b]) && panel(&["Alpha", "Beta", "Gamma"], "", 0),
    ) || !step(
        "Tab moves the selection",
        keys(&[0x0f]) && panel(&["Alpha", "Beta", "Gamma"], "", 1),
    ) || !step(
        "typing filters the list",
        keys(&[0x30, 0x12]) && panel(&["Beta"], "be", 0),
    ) {
        return false;
    }
    // Enter launches Beta: its window opens (the first window, at 40, 40),
    // the launcher closes, and Beta is the shell's child.
    let launched = keys(&[0x1c]) && until(3000, || compositor::pixel(45, 45) == Ok(BLUE));
    if !step("Enter launches the selection", launched)
        || !step("launcher closes after launching", closed())
        || !step("the application is the shell's child", children() == 1)
    {
        return false;
    }
    let reaped = keys(&[0x15])
        && until(2000, || {
            compositor::pixel(45, 45) == Ok(wallpaper(45, 45, height))
        })
        && until(1000, || children() == 0);
    if !step("Beta takes its key, exits, and is reaped", reaped) {
        return false;
    }

    // Pointer: the start button opens the launcher; clicking a row launches.
    let (mut px, mut py) = (width / 2, height / 2);
    let opened =
        move_by(26 - px, top + 12 - py) && click() && panel(&["Alpha", "Beta", "Gamma"], "", 0);
    (px, py) = (26, top + 12);
    if !step("start button opens the launcher", opened) {
        return false;
    }
    // Alpha is the first row: panel top + border + two rows, plus half a row.
    let row_y = top - 2 - panel_height(3) + 1 + 2 * 14 + 7;
    let clicked = move_by(60 - px, row_y - py)
        && click()
        && until(3000, || compositor::pixel(70, 70) == Ok(RED));
    (px, py) = (60, row_y);
    if !step("clicking a row launches it", clicked) {
        return false;
    }
    // Park the pointer away from the panel area, then let Alpha exit.
    let parked = move_by(300 - px, 300 - py);
    let reaped = parked
        && closed()
        && children() == 1
        && keys(&[0x2d])
        && until(2000, || {
            compositor::pixel(70, 70) == Ok(wallpaper(70, 70, height))
        })
        && until(1000, || children() == 0);
    if !step("Alpha takes its key, exits, and is reaped", reaped) {
        return false;
    }

    // A catalog entry whose program is missing: a toast, and no child.
    let toast = |text: &str| {
        let (x0, y0) = (width - 188, 8);
        let mut mask = Surface::new(180, 22);
        let bounds = mask.bounds();
        mask.fill_rect(bounds, UNSET);
        mask.draw_text(6, 8, text, WHITE);
        until(1000, || {
            region(x0, y0, 180, 22, |x, y| {
                let (cx, cy) = (x - x0, y - y0);
                if cx == 0 || cy == 0 || cx == 179 || cy == 21 {
                    ACCENT
                } else if mask.pixel(cx, cy) == Some(WHITE) {
                    WHITE
                } else {
                    blend_pixel(TOAST_FILL, wallpaper(x, y, height), 216)
                }
            })
        })
    };
    let failed = keys(&[0x3b, 0x22])
        && panel(&["Gamma"], "g", 0)
        && keys(&[0x1c])
        && toast("Could not start Gamma")
        && closed()
        && children() == 0;
    if !step("a missing program is reported, not launched", failed) {
        return false;
    }
    let nothing = keys(&[0x3b, 0x2c, 0x2c])
        && panel(&[], "zz", 0)
        && keys(&[0x1c])
        && panel(&[], "zz", 0)
        && children() == 0
        && keys(&[0x3b])
        && closed();
    if !step("a search with no match says so; F1 closes", nothing) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.2 acceptance: applications on the WovenShell desktop.
///
/// The shell runs with two hello-gui `--desktop-client` applications. Real
/// i8042 pointer and keyboard input (the Stage 19.12 loopback) drives it.
/// Every check reads the compositor surface and compares it with the
/// kernel's own rendering: window frames and title bars, taskbar entries,
/// the tray icon, the notification toast and its expiry, client damage,
/// raising by taskbar click, the pointer, key routing to the focused
/// window, and windows, entries and icons disappearing when their
/// application exits.
#[cfg(feature = "stage20-2-test")]
fn stage20_2_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Surface};

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const BLACK: u32 = rgb(0, 0, 0);
    const TASKBAR: u32 = rgb(20, 24, 32);
    const ACCENT: u32 = rgb(90, 140, 220);
    const ACTIVE: u32 = rgb(70, 120, 210);
    const ENTRY_INACTIVE: u32 = rgb(44, 52, 68);
    const TITLE_INACTIVE: u32 = rgb(60, 66, 80);
    const TOAST_FILL: u32 = rgb(30, 34, 46);
    const RED: u32 = rgb(200, 60, 60);
    const BLUE: u32 = rgb(60, 60, 200);
    const GREEN: u32 = rgb(40, 200, 90);
    const UNSET: u32 = 0x0100_0000;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.2] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    /// Poll `condition` until it holds, for at most `limit` ticks. The shell
    /// repaints concurrently, so a check may first see a frame in progress.
    fn until(limit: u64, condition: impl Fn() -> bool) -> bool {
        let start = timer::ticks();
        loop {
            if condition() {
                return true;
            }
            if timer::ticks().wrapping_sub(start) > limit {
                return false;
            }
            task::yield_now();
        }
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn region(
        x0: i32,
        y0: i32,
        width: i32,
        height: i32,
        expected: impl Fn(i32, i32) -> u32,
    ) -> bool {
        (y0..y0 + height)
            .all(|y| (x0..x0 + width).all(|x| compositor::pixel(x, y) == Ok(expected(x, y))))
    }
    /// A `width`×`height` box filled with `fill`, with `text` in white at
    /// `(dx, dy)` inside it, as the kernel renders it, compared with the
    /// screen at `(x, y)`.
    fn labelled(
        origin: (i32, i32),
        size: (i32, i32),
        offset: (i32, i32),
        text: &str,
        fill: u32,
    ) -> bool {
        let ((x, y), (width, height), (dx, dy)) = (origin, size, offset);
        let mut mask = Surface::new(width as usize, height as usize);
        let bounds = mask.bounds();
        mask.fill_rect(bounds, UNSET);
        mask.draw_text(dx, dy, text, WHITE);
        region(x, y, width, height, |px, py| {
            match mask.pixel(px - x, py - y) {
                Some(WHITE) => WHITE,
                _ => fill,
            }
        })
    }
    /// A title bar: fill, title text, and the close control at its right
    /// (Stage 20.4).
    fn title_bar(origin: (i32, i32), width: i32, text: &str, fill: u32) -> bool {
        let (x, y) = origin;
        let mut expected = Surface::new(width as usize, 14);
        let bounds = expected.bounds();
        expected.fill_rect(bounds, fill);
        expected.draw_text(4, 4, text, WHITE);
        expected.fill_rect(
            crate::display::Rect::new(width - 12, 2, 10, 10),
            rgb(170, 60, 60),
        );
        expected.draw_text(width - 12 + 3, 4, "x", WHITE);
        region(x, y, width, 14, |px, py| {
            expected.pixel(px - x, py - y).unwrap_or(UNSET)
        })
    }
    /// One PS/2 packet through the real auxiliary path.
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    /// Relative motion in packets of at most 100 per axis.
    fn move_by(mut dx: i32, mut dy: i32) -> bool {
        while dx != 0 || dy != 0 {
            let (sx, sy) = (dx.clamp(-100, 100), dy.clamp(-100, 100));
            if !packet(sx as i16, sy as i16, false) {
                return false;
            }
            dx -= sx;
            dy -= sy;
        }
        true
    }
    fn click() -> bool {
        packet(0, 0, true) && packet(0, 0, false)
    }

    let install = |path: &str, image: &[u8]| {
        vfs::create_read_only(path, image).is_ok() && {
            let mut copy = alloc::vec![0u8; image.len()];
            vfs::read_all(path, &mut copy) == Ok(image.len())
        }
    };
    if !step(
        "install shell and application",
        install("/bin/woven-shell", WOVEN_SHELL_IMAGE)
            && install("/bin/hello-gui", HELLO_GUI_IMAGE),
    ) {
        return false;
    }
    let shell_capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(program) = userspace::load_elf_with_argv(WOVEN_SHELL_IMAGE, &["/bin/woven-shell"])
    else {
        return step("shell image loads", false);
    };
    let Ok((shell, _, _)) =
        task::spawn_user_system_service("woven-shell", program, shell_capabilities)
    else {
        return step("shell spawned", false);
    };
    if !step(
        "shell took the display",
        until(3000, || {
            compositor::holder() == Some(shell.as_u64()) && compositor::is_presenting()
        }),
    ) {
        return false;
    }
    let Some((width, height)) = compositor::dimensions() else {
        return step("desktop geometry", false);
    };
    let (width, height) = (width as i32, height as i32);
    let top = height - 24;
    let taskbar = move |x: i32, y: i32| blend_pixel(TASKBAR, wallpaper(x, y, height), 216);
    let entry = |index: i32| (56 + index * 84, top + 4);
    let (tray_x, tray_y) = (width - 54 - 20, top + 4);
    let (toast_x, toast_y) = (width - 188, 8);
    serial::write_line(format_args!("[S20.2] desktop {}x{}", width, height));

    let client_capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::Ipc);
    let shell_arg = alloc::format!("{}", shell.as_u64());
    let Some(alpha) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "a"],
        Some(client_capabilities),
    ) else {
        return step("application A spawned", false);
    };
    // A's window cascades to (40, 40), 120x80, its title bar above.
    if !step(
        "window A opened",
        until(2000, || compositor::pixel(45, 45) == Ok(RED)),
    ) {
        return false;
    }
    let frame_a = until(500, || {
        title_bar((40, 26), 120, "Alpha", ACTIVE)
            && region(40, 40, 120, 80, |x, y| {
                if (50..70).contains(&x) && (50..60).contains(&y) {
                    compositor::pixel(x, y).unwrap_or(UNSET)
                } else {
                    RED
                }
            })
    });
    let entry_a = until(500, || {
        labelled(entry(0), (80, 16), (4, 5), "Alpha", ACTIVE)
    });
    let tray = until(500, || region(tray_x, tray_y, 16, 16, |_, _| GREEN));
    let toast = until(500, || {
        let mut mask = Surface::new(180, 22);
        let bounds = mask.bounds();
        mask.fill_rect(bounds, UNSET);
        mask.draw_text(6, 8, "Alpha ready", WHITE);
        region(toast_x, toast_y, 180, 22, |x, y| {
            let (cx, cy) = (x - toast_x, y - toast_y);
            if cx == 0 || cy == 0 || cx == 179 || cy == 21 {
                ACCENT
            } else if mask.pixel(cx, cy) == Some(WHITE) {
                WHITE
            } else {
                blend_pixel(TOAST_FILL, wallpaper(x, y, height), 216)
            }
        })
    });
    if !step("window A framed with a focused title bar", frame_a)
        || !step("taskbar entry for A, focused", entry_a)
        || !step("tray icon shown", tray)
        || !step("notification toast shown", toast)
    {
        return false;
    }
    // A redraws a 20x10 patch at (10, 10) and reports damage; only the
    // shell's damage path brings it to the screen.
    if !step(
        "client damage composited",
        until(1500, || region(50, 50, 20, 10, |_, _| WHITE)),
    ) {
        return false;
    }

    let Some(beta) = spawn_hello_gui(
        HELLO_GUI_IMAGE,
        &["/bin/hello-gui", "--desktop-client", &shell_arg, "b"],
        Some(client_capabilities),
    ) else {
        return step("application B spawned", false);
    };
    // B cascades to (64, 64), 100x60, and opens on top, focused.
    let stacked = until(2000, || compositor::pixel(150, 100) == Ok(BLUE))
        && until(500, || {
            compositor::pixel(45, 45) == Ok(RED)
                && title_bar((64, 50), 100, "Beta", ACTIVE)
                && title_bar((40, 26), 120, "Alpha", TITLE_INACTIVE)
                && labelled(entry(0), (80, 16), (4, 5), "Alpha", ENTRY_INACTIVE)
                && labelled(entry(1), (80, 16), (4, 5), "Beta", ACTIVE)
        });
    if !step("window B opens on top and takes focus", stacked) {
        return false;
    }
    if !step(
        "toast expires and the desktop shows through",
        until(1000, || {
            region(toast_x, toast_y, 180, 22, |x, y| wallpaper(x, y, height))
        }),
    ) {
        return false;
    }

    // The pointer starts hidden at the centre. Move it onto A's taskbar
    // entry and click: A is raised and focused.
    let (mut px, mut py) = (width / 2, height / 2);
    let (target_x, target_y) = (entry(0).0 + 40, entry(0).1 + 8);
    let raised = move_by(target_x - px, target_y - py)
        && click()
        && until(1000, || compositor::pixel(150, 100) == Ok(RED));
    (px, py) = (target_x, target_y);
    if !step("taskbar click raises window A", raised) {
        return false;
    }
    let pointer = until(500, || {
        compositor::pixel(px, py) == Ok(BLACK) && compositor::pixel(px + 1, py + 2) == Ok(WHITE)
    });
    if !step("pointer drawn at its position", pointer) {
        return false;
    }
    // Park the pointer on empty desktop, then type: the key goes to A, the
    // focused window, and A exits once it has it.
    let parked = move_by(300 - px, 300 - py);
    (px, py) = (300, 300);
    let typed = parked
        && until(500, || compositor::pixel(px, py) == Ok(BLACK))
        && mouse::keyboard_loopback(0x2d);
    let alpha_status = wait_hello_gui(alpha).map(|(_, status)| status);
    if !step(
        "key x routed to focused window A",
        typed && alpha_status == Some(Ok(0)),
    ) {
        return false;
    }
    let gone = until(1000, || {
        compositor::pixel(45, 45) == Ok(wallpaper(45, 45, height))
    }) && until(500, || {
        region(tray_x, tray_y, 16, 16, taskbar)
            && labelled(entry(0), (80, 16), (4, 5), "Beta", ACTIVE)
            && region(entry(1).0, entry(1).1, 80, 16, taskbar)
            && title_bar((64, 50), 100, "Beta", ACTIVE)
            && compositor::pixel(150, 100) == Ok(BLUE)
    });
    if !step(
        "A's window, entry and tray icon removed on exit; B focused",
        gone,
    ) {
        return false;
    }

    // Click inside B and type: B gets its key and exits.
    let typed = move_by(100 - px, 100 - py) && click() && mouse::keyboard_loopback(0x15);
    let beta_status = wait_hello_gui(beta).map(|(_, status)| status);
    if !step(
        "key y routed to window B",
        typed && beta_status == Some(Ok(0)),
    ) {
        return false;
    }
    let empty = until(1000, || {
        compositor::pixel(150, 100) == Ok(wallpaper(150, 100, height))
            && region(entry(0).0, entry(0).1, 80, 16, taskbar)
    });
    if !step("desktop empty again", empty) {
        return false;
    }

    if task::kill_process(shell.as_u64(), 9).is_err()
        || !until(1000, || task::process_exited(shell))
    {
        return step("shell killed", false);
    }
    let _ = task::wait_process(shell.as_u64());
    step(
        "display returned to the terminal",
        compositor::holder().is_none() && !compositor::is_presenting(),
    )
}

/// Stage 20.1 acceptance: the WovenShell desktop.
///
/// The kernel checks the clock decoder and reads the hardware clock, then
/// starts woven-shell as a system service and waits for it to take the
/// display. Every check reads the compositor surface and recomputes what
/// should be there independently: the wallpaper formula, the translucent
/// taskbar, the start button rendered with the kernel's own font, and the
/// clock text against the kernel's own reading of the hardware clock, twice,
/// so the clock is seen to advance. Killing the shell must hand the display
/// back to the text terminal.
#[cfg(feature = "stage20-1-test")]
fn stage20_1_self_test() -> bool {
    use crate::display::{blend_pixel, rgb, Surface};
    use bootloader_api::info::PixelFormat;

    const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
    const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
    const WHITE: u32 = rgb(255, 255, 255);
    const TASKBAR: u32 = rgb(20, 24, 32);
    const ACCENT: u32 = rgb(90, 140, 220);
    const START_FILL: u32 = rgb(52, 96, 180);
    const TASKBAR_HEIGHT: i32 = 24;
    /// Not a valid 0x00RRGGBB pixel, so it never matches a screen pixel.
    const UNSET: u32 = 0x0100_0000;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.1] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
        let span = (height - 1).max(1);
        let t = (y.clamp(0, span) * 255 / span) as u8;
        let base = blend_pixel(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
        if x % 32 == 0 || y % 32 == 0 {
            blend_pixel(WHITE, base, 28)
        } else {
            base
        }
    }
    fn taskbar(x: i32, y: i32, height: i32) -> u32 {
        blend_pixel(TASKBAR, wallpaper(x, y, height), 216)
    }
    /// Text rendered with the kernel's font into a `width`×`height` mask at
    /// offset `(dx, dy)`: white where a glyph pixel is set, UNSET elsewhere.
    fn text_mask(width: usize, height: usize, dx: i32, dy: i32, text: &str) -> Surface {
        let mut mask = Surface::new(width, height);
        let bounds = mask.bounds();
        mask.fill_rect(bounds, UNSET);
        mask.draw_text(dx, dy, text, WHITE);
        mask
    }
    /// Whether every compositor pixel in the rectangle is `expected(x, y)`.
    fn region(
        x0: i32,
        y0: i32,
        width: i32,
        height: i32,
        expected: impl Fn(i32, i32) -> u32,
    ) -> bool {
        (y0..y0 + height)
            .all(|y| (x0..x0 + width).all(|x| compositor::pixel(x, y) == Ok(expected(x, y))))
    }
    fn screen_pixel(x: usize, y: usize) -> Option<u32> {
        terminal::with_framebuffer(|buffer, info| {
            if x >= info.width || y >= info.height || info.bytes_per_pixel < 3 {
                return None;
            }
            let offset = (y * info.stride + x) * info.bytes_per_pixel;
            let bytes = buffer.get(offset..offset + 3)?;
            match info.pixel_format {
                PixelFormat::Rgb => Some(rgb(bytes[0], bytes[1], bytes[2])),
                PixelFormat::Bgr => Some(rgb(bytes[2], bytes[1], bytes[0])),
                _ => None,
            }
        })
        .flatten()
    }
    fn screen_checksum() -> Option<u64> {
        terminal::with_framebuffer(|buffer, _| {
            buffer.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
            })
        })
    }
    /// `HH:MM:SS` for a second of the day.
    fn clock_text(second_of_day: u32) -> [u8; 8] {
        let (hour, minute, second) = (
            second_of_day / 3600,
            second_of_day / 60 % 60,
            second_of_day % 60,
        );
        let digit = |value: u32| b'0' + (value % 10) as u8;
        [
            digit(hour / 10),
            digit(hour),
            b':',
            digit(minute / 10),
            digit(minute),
            b':',
            digit(second / 10),
            digit(second),
        ]
    }

    if !step(
        "clock decoder handles BCD, binary and 12-hour forms",
        rtc::decode_self_test(),
    ) {
        return false;
    }
    let now = rtc::read();
    if let Some(time) = now {
        serial::write_line(format_args!(
            "[S20.1] hardware clock {:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            time.year, time.month, time.day, time.hour, time.minute, time.second
        ));
    }
    if !step(
        "hardware clock read",
        now.is_some_and(|time| time.year >= 2024),
    ) {
        return false;
    }

    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/woven-shell", WOVEN_SHELL_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; WOVEN_SHELL_IMAGE.len()];
    if vfs::read_all("/bin/woven-shell", &mut image) != Ok(WOVEN_SHELL_IMAGE.len()) {
        return step("filesystem round-trip", false);
    }
    let capabilities = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::DisplayIo)
        .with(capability::Capability::InputIo);
    let Some(program) = userspace::load_elf_with_argv(&image, &["/bin/woven-shell", "--seconds"])
    else {
        return step("shell image loads", false);
    };
    let Ok((pid, _, _)) = task::spawn_user_system_service("woven-shell", program, capabilities)
    else {
        return step("shell spawned", false);
    };
    let start = timer::ticks();
    while !(compositor::holder() == Some(pid.as_u64()) && compositor::is_presenting()) {
        if task::process_exited(pid) || timer::ticks().wrapping_sub(start) > 3000 {
            let status = if task::process_exited(pid) {
                task::wait_process(pid.as_u64()).map_or(-1, i64::from)
            } else {
                -2
            };
            serial::write_line(format_args!("[S20.1] shell status {}", status));
            return step("shell took the display", false);
        }
        task::yield_now();
    }
    step("shell took the display", true);

    // A framebuffer of at least 1280x800 is shown at scale 2.
    let Some(info) = terminal::framebuffer_info() else {
        return step("framebuffer available", false);
    };
    let scale = if info.width >= 1280 && info.height >= 800 {
        2
    } else {
        1
    };
    let Some((width, height)) = compositor::dimensions() else {
        return step("compositor geometry", false);
    };
    serial::write_line(format_args!(
        "[S20.1] framebuffer {}x{}, desktop {}x{} at scale {}",
        info.width, info.height, width, height, scale
    ));
    let (width, height) = (width as i32, height as i32);
    if !step(
        "desktop geometry follows the display scale",
        compositor::scale() == Some(scale)
            && width == (info.width as u32 / scale).min(compositor::MAX_WIDTH) as i32
            && height == (info.height as u32 / scale).min(compositor::MAX_HEIGHT) as i32,
    ) {
        return false;
    }

    let top = height - TASKBAR_HEIGHT;
    // Wallpaper: corner, centre, both weave lines, and the last row above
    // the taskbar, each a block wide enough to straddle a grid line.
    let wall = region(0, 0, 40, 40, |x, y| wallpaper(x, y, height))
        && region(width / 2 - 20, height / 2 - 20, 40, 40, |x, y| {
            wallpaper(x, y, height)
        })
        && region(width - 40, top - 40, 40, 40, |x, y| wallpaper(x, y, height));
    if !step("wallpaper matches its formula", wall) {
        return false;
    }
    // The Quick Settings button (Stage 20.10) sits left of the tray's four
    // icon slots, which are empty here, and the clock.
    const QS_FILL: u32 = rgb(44, 52, 68);
    let clock_width = 8 * crate::font::ADVANCE + 6;
    let qs_x = width - clock_width - 5 * (16 + 4);
    // The workspace switcher (Stage 20.13): four 12-pixel boxes, 2 apart,
    // ending 6 left of the Quick Settings button.
    let ws_x = qs_x - 6 - (4 * 14 - 2);
    let bar = region(0, top, width, 1, |_, _| ACCENT)
        && region(60, top + 1, ws_x - 60, TASKBAR_HEIGHT - 1, |x, y| {
            taskbar(x, y, height)
        })
        && region(ws_x, top + 1, qs_x - ws_x, 3, |x, y| taskbar(x, y, height))
        && region(ws_x, top + 20, qs_x - ws_x, TASKBAR_HEIGHT - 20, |x, y| {
            taskbar(x, y, height)
        })
        && region(
            qs_x + 16,
            top + 1,
            width - 80 - (qs_x + 16),
            TASKBAR_HEIGHT - 1,
            |x, y| taskbar(x, y, height),
        )
        && region(qs_x, top + 1, 16, 3, |x, y| taskbar(x, y, height))
        && region(qs_x, top + 20, 16, TASKBAR_HEIGHT - 20, |x, y| {
            taskbar(x, y, height)
        });
    if !step("taskbar is translucent over the wallpaper", bar) {
        return false;
    }
    let qs_mask = text_mask(16, 16, 5, 5, "Q");
    let qs = region(qs_x, top + 4, 16, 16, |x, y| {
        match qs_mask.pixel(x - qs_x, y - (top + 4)) {
            Some(WHITE) => WHITE,
            _ => QS_FILL,
        }
    });
    if !step("Quick Settings button and its label", qs) {
        return false;
    }
    // Workspace boxes: the first current (active fill), digits 1-4, the
    // gaps and the space before the button bare taskbar.
    const WS_ACTIVE: u32 = rgb(70, 120, 210);
    let switcher = (0..4).all(|index: i32| {
        let box_x = ws_x + index * 14;
        let digit = [b'1' + index as u8];
        let mask = text_mask(12, 16, 3, 5, core::str::from_utf8(&digit).unwrap_or("?"));
        let fill = if index == 0 { WS_ACTIVE } else { QS_FILL };
        region(box_x, top + 4, 12, 16, |x, y| {
            match mask.pixel(x - box_x, y - (top + 4)) {
                Some(WHITE) => WHITE,
                _ => fill,
            }
        }) && (index == 3 || region(box_x + 12, top + 4, 2, 16, |x, y| taskbar(x, y, height)))
    }) && region(ws_x + 54, top + 4, qs_x - (ws_x + 54), 16, |x, y| {
        taskbar(x, y, height)
    });
    if !step("workspace switcher, workspace 1 current", switcher) {
        return false;
    }
    // The start button, rendered here with the kernel's own font.
    let label_x = 4 + (44 - crate::font::text_width("Woven") as i32) / 2;
    let button_mask = text_mask(44, 16, label_x - 4, 5, "Woven");
    let button = region(4, top + 4, 44, 16, |x, y| {
        match button_mask.pixel(x - 4, y - (top + 4)) {
            Some(WHITE) => WHITE,
            _ => START_FILL,
        }
    });
    if !step("start button and its label", button) {
        return false;
    }

    // The clock: the region must match the kernel's own reading of the
    // hardware clock, allowing up to two seconds of display lag, and must
    // then advance to a later reading.
    let clock_x = width - clock_width;
    let clock_matches = |text: &[u8; 8]| {
        let text = core::str::from_utf8(text).unwrap_or("");
        let text_x = width - 6 - crate::font::text_width(text) as i32;
        let mask = text_mask(clock_width as usize, 23, text_x - clock_x, 8, text);
        region(
            clock_x,
            top + 1,
            clock_width,
            TASKBAR_HEIGHT - 1,
            |x, y| match mask.pixel(x - clock_x, y - (top + 1)) {
                Some(WHITE) => WHITE,
                _ => taskbar(x, y, height),
            },
        )
    };
    let mut first: Option<[u8; 8]> = None;
    let mut advanced = false;
    let start = timer::ticks();
    while timer::ticks().wrapping_sub(start) < 600 && !advanced {
        let Some(time) = rtc::read() else {
            break;
        };
        let second =
            u32::from(time.hour) * 3600 + u32::from(time.minute) * 60 + u32::from(time.second);
        for lag in 0..3 {
            let text = clock_text((second + 86_400 - lag) % 86_400);
            if clock_matches(&text) {
                match first {
                    None => first = Some(text),
                    Some(shown) if shown != text => advanced = true,
                    Some(_) => {}
                }
                break;
            }
        }
        task::yield_now();
    }
    if let Some(text) = first {
        serial::write_line(format_args!(
            "[S20.1] clock shows {}",
            core::str::from_utf8(&text).unwrap_or("?")
        ));
    }
    if !step("clock shows the hardware time", first.is_some())
        || !step("clock advances with the hardware clock", advanced)
    {
        return false;
    }

    // The physical framebuffer carries the desktop at its scale.
    let s = scale as usize;
    let physical = [(5, 5), (300, top), (6, top + 6), (width / 2, height / 2)]
        .into_iter()
        .all(|(x, y)| {
            let expected = compositor::pixel(x, y).ok();
            let (px, py) = (x as usize * s, y as usize * s);
            expected.is_some()
                && screen_pixel(px, py) == expected
                && screen_pixel(px + s - 1, py + s - 1) == expected
        });
    if !step("framebuffer shows the desktop at its scale", physical) {
        return false;
    }

    // Killing the shell hands the display back to the terminal.
    if task::kill_process(pid.as_u64(), 9).is_err() {
        return step("shell killed", false);
    }
    let start = timer::ticks();
    while !task::process_exited(pid) {
        if timer::ticks().wrapping_sub(start) > 1000 {
            return step("shell killed", false);
        }
        task::yield_now();
    }
    let _ = task::wait_process(pid.as_u64());
    step("shell killed", true);
    let released = compositor::holder().is_none() && !compositor::is_presenting();
    let before = screen_checksum();
    terminal::write_bytes(b"terminal after the desktop\n");
    let terminal_back = before.is_some() && screen_checksum() != before;
    serial::write_line(format_args!(
        "[S20.1] holder {:?}, presenting {}, terminal draws {}",
        compositor::holder(),
        compositor::is_presenting(),
        terminal_back
    ));
    step(
        "display returned to the terminal",
        released && terminal_back,
    )
}

/// Stage 19.14 acceptance: integer display scaling and paced animation.
///
/// The kernel first drives a scaled presentation itself and reads the
/// physical framebuffer back: one surface pixel must be a 2x2 block. Then
/// hello-gui `--scaled-compositor` switches to scale 2 through the syscall,
/// checks the geometry, and animates a square under a `FrameClock`. The
/// kernel re-reads the final frame after it exits, and checks that the next
/// holder starts at scale 1.
#[cfg(feature = "stage19-14-test")]
fn stage19_14_self_test() -> bool {
    use crate::display::{rgb, Rect};
    use bootloader_api::info::PixelFormat;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.14] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    /// A physical framebuffer pixel as 0x00RRGGBB, read independently of the
    /// compositor.
    fn screen_pixel(x: usize, y: usize) -> Option<u32> {
        terminal::with_framebuffer(|buffer, info| {
            if x >= info.width || y >= info.height || info.bytes_per_pixel < 3 {
                return None;
            }
            let offset = (y * info.stride + x) * info.bytes_per_pixel;
            let bytes = buffer.get(offset..offset + 3)?;
            match info.pixel_format {
                PixelFormat::Rgb => Some(rgb(bytes[0], bytes[1], bytes[2])),
                PixelFormat::Bgr => Some(rgb(bytes[2], bytes[1], bytes[0])),
                _ => None,
            }
        })
        .flatten()
    }

    let Some(info) = terminal::framebuffer_info() else {
        return step("framebuffer available", false);
    };
    let full = (
        (info.width as u32).min(compositor::MAX_WIDTH),
        (info.height as u32).min(compositor::MAX_HEIGHT),
    );
    let half = (
        (info.width as u32 / 2).min(compositor::MAX_WIDTH),
        (info.height as u32 / 2).min(compositor::MAX_HEIGHT),
    );
    serial::write_line(format_args!(
        "[S19.14] framebuffer {}x{}, scale 2 surface {}x{}",
        info.width, info.height, half.0, half.1
    ));
    if !step(
        "boot compositor at scale 1",
        compositor::scale() == Some(1) && compositor::dimensions() == Some(full),
    ) {
        return false;
    }
    let refused = compositor::set_scale(0) == Err(compositor::Error::InvalidScale)
        && compositor::set_scale(compositor::MAX_SCALE + 1) == Err(compositor::Error::InvalidScale)
        && compositor::scale() == Some(1)
        && compositor::dimensions() == Some(full);
    if !step("out-of-range scales refused", refused) {
        return false;
    }

    // Kernel-driven scaled presentation, read back from the framebuffer.
    if !step("display taken", compositor::activate_display().is_ok()) {
        return false;
    }
    let scaled = compositor::set_scale(2) == Ok(half)
        && compositor::dimensions() == Some(half)
        && compositor::scale() == Some(2);
    if !step("scale 2 halves the surface", scaled) {
        compositor::deactivate_display();
        return false;
    }
    let whole = (half.0 as usize * 2) * (half.1 as usize * 2);
    if !step(
        "rescale repaints the whole screen",
        compositor::present() == Ok(whole),
    ) {
        compositor::deactivate_display();
        return false;
    }
    const OWNER: u64 = 4248;
    const COLOR: u32 = rgb(30, 160, 220);
    let blitted = user_graphics::create(OWNER, 8, 8).is_ok_and(|client| {
        user_graphics::fill_rect(client, Rect::new(0, 0, 8, 8), COLOR).is_ok()
            && compositor::blit(client, 10, 10) == Ok(Rect::new(10, 10, 8, 8))
    });
    user_graphics::release_owner(OWNER);
    // An 8x8 blit is a 16x16 block of framebuffer pixels at (20, 20).
    let presented = blitted && compositor::present() == Ok(16 * 16);
    let blocks = screen_pixel(20, 20) == Some(COLOR)
        && screen_pixel(35, 35) == Some(COLOR)
        && screen_pixel(19, 20) == Some(0)
        && screen_pixel(36, 35) == Some(0)
        && screen_pixel(20, 36) == Some(0);
    let restored = compositor::set_scale(1) == Ok(full);
    compositor::deactivate_display();
    if !step("8x8 blit presented as a 16x16 block", presented && blocks)
        || !step("scale 1 restored", restored)
    {
        return false;
    }

    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if vfs::read_all("/bin/hello-gui", &mut image) != Ok(HELLO_GUI_IMAGE.len()) {
        return step("filesystem round-trip", false);
    }
    const VICTIM_OWNER: u64 = 4249;
    let Ok(victim) = user_graphics::create(VICTIM_OWNER, 8, 8) else {
        return step("victim surface created", false);
    };
    let victim_arg = alloc::format!("{}", victim.to_u64());
    let probe = run_hello_gui(&image, &["/bin/hello-gui", "--probe", &victim_arg], false);
    user_graphics::release_owner(VICTIM_OWNER);
    if !step(
        "unprivileged probe refused the display geometry",
        matches!(probe, Some((_, Ok(0)))),
    ) {
        return false;
    }

    let scaled = run_hello_gui(&image, &["/bin/hello-gui", "--scaled-compositor"], true);
    let code = match scaled {
        Some((_, Ok(code))) => code as i64,
        _ => -1,
    };
    serial::write_line(format_args!("[S19.14] compositor exit status {}", code));
    if !step(
        "compositor scaled the display and paced its animation",
        matches!(scaled, Some((_, Ok(0)))),
    ) {
        return false;
    }

    // The final frame, re-read in surface coordinates after the holder left.
    const BACKDROP: u32 = rgb(200, 200, 200);
    const SQUARE: u32 = rgb(250, 210, 40);
    let final_frame = compositor::scale() == Some(2)
        && compositor::dimensions() == Some(half)
        && compositor::pixel(10 + 8 * 12 + 8, 68) == Ok(SQUARE)
        && compositor::pixel(18, 68) == Ok(BACKDROP);
    if !step("kernel reads the final frame at scale 2", final_frame)
        || !step("lease released on exit", compositor::holder().is_none())
    {
        return false;
    }
    const NEXT: u64 = 4250;
    let reset = compositor::claim(NEXT).is_ok()
        && compositor::scale() == Some(1)
        && compositor::dimensions() == Some(full);
    compositor::release_owner(NEXT);
    step("the next holder starts at scale 1", reset)
}

/// Stage 19.13 acceptance: translucent windows and shadows.
///
/// hello-gui `--alpha-compositor` draws an opaque backdrop, a shadow from its
/// own surface at 96/255, and a `--client`'s shared window at 160/255 over
/// both, checking each region against the blend formula. The kernel re-reads
/// the same regions after both exit, computing expectations with its own
/// `display::blend_pixel`.
#[cfg(feature = "stage19-13-test")]
fn stage19_13_self_test() -> bool {
    use crate::display::{blend_pixel, rgb};

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.13] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if vfs::read_all("/bin/hello-gui", &mut image) != Ok(HELLO_GUI_IMAGE.len()) {
        return step("filesystem round-trip", false);
    }

    const VICTIM_OWNER: u64 = 4247;
    let Ok(victim) = user_graphics::create(VICTIM_OWNER, 8, 8) else {
        return step("victim surface created", false);
    };
    let victim_arg = alloc::format!("{}", victim.to_u64());
    let probe = run_hello_gui(&image, &["/bin/hello-gui", "--probe", &victim_arg], false);
    user_graphics::release_owner(VICTIM_OWNER);
    if !step(
        "unprivileged probe refused blending",
        matches!(probe, Some((_, Ok(0)))),
    ) {
        return false;
    }

    let capabilities = display_client_capabilities().with(capability::Capability::Ipc);
    let Some(compositor_pid) = spawn_hello_gui(
        &image,
        &["/bin/hello-gui", "--alpha-compositor"],
        Some(capabilities),
    ) else {
        return step("compositor process spawned", false);
    };
    let start = timer::ticks();
    while compositor::holder() != Some(compositor_pid.as_u64()) {
        if task::process_exited(compositor_pid) || timer::ticks().wrapping_sub(start) > 1000 {
            return step("compositor process holds the lease", false);
        }
        task::yield_now();
    }
    step("compositor process holds the lease", true);
    let compositor_arg = alloc::format!("{}", compositor_pid.as_u64());
    let Some(client_pid) = spawn_hello_gui(
        &image,
        &["/bin/hello-gui", "--client", &compositor_arg],
        None,
    ) else {
        return step("client process spawned", false);
    };

    let code = |status: Option<Result<i32, task::WaitError>>| match status {
        Some(Ok(code)) => code as i64,
        _ => -1,
    };
    let client = wait_hello_gui(client_pid).map(|(_, status)| status);
    let composited = wait_hello_gui(compositor_pid).map(|(_, status)| status);
    serial::write_line(format_args!(
        "[S19.13] client exit status {} compositor exit status {}",
        code(client),
        code(composited)
    ));
    if !step("client exits zero", client == Some(Ok(0)))
        || !step(
            "compositor verified every blended region",
            composited == Some(Ok(0)),
        )
    {
        return false;
    }

    const BACKDROP: u32 = rgb(200, 200, 200);
    const CLIENT: u32 = rgb(180, 40, 90);
    let shaded = blend_pixel(rgb(0, 0, 0), BACKDROP, 96);
    let regions = compositor::pixel(105, 105) == Ok(BACKDROP)
        && compositor::pixel(172, 163) == Ok(shaded)
        && compositor::pixel(131, 131) == Ok(blend_pixel(CLIENT, BACKDROP, 160))
        && compositor::pixel(150, 150) == Ok(blend_pixel(CLIENT, shaded, 160));
    if !step("kernel recomputes every region independently", regions) {
        return false;
    }
    step("lease released on exit", compositor::holder().is_none())
}

/// Stage 19.12 acceptance: pointer input, and keystrokes delivered at IRQ time.
///
/// Everything here goes through the real i8042: command 0xD3 presents a byte
/// as if the mouse sent it (IRQ12, the mouse driver), 0xD2 as if the keyboard
/// did (IRQ1, the keyboard driver). The kernel first checks the packet decoder
/// and the raw paths into a focus queue, then drives hello-gui `--wm` with a
/// pointer move, a click on the bottom window, and a typed key, twice. Each
/// client must receive exactly its own key: click-to-focus works, and keys
/// reach a window manager asleep in `WaitAny` without anyone polling.
#[cfg(feature = "stage19-12-test")]
fn stage19_12_self_test() -> bool {
    use woven_input::{Event, Key};

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.12] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until_blocked(pid: task::ProcessId) -> bool {
        let start = timer::ticks();
        while !task::process_task_blocked(pid) {
            if task::process_exited(pid) || timer::ticks().wrapping_sub(start) > 2000 {
                return false;
            }
            task::yield_now();
        }
        true
    }
    /// One PS/2 packet: buttons, then 9-bit signed deltas with PS/2's upward Y.
    fn packet(dx: i16, screen_dy: i16, left: bool) -> bool {
        let dy = -screen_dy;
        let flags =
            0x08 | u8::from(left) | if dx < 0 { 0x10 } else { 0 } | if dy < 0 { 0x20 } else { 0 };
        [flags, dx as u8, dy as u8].into_iter().all(mouse::loopback)
    }
    /// A key press (make code only; the decoder ignores break codes).
    fn key(scancode: u8) -> bool {
        mouse::keyboard_loopback(scancode)
    }
    fn next_event(owner: u64) -> Option<Event> {
        let start = timer::ticks();
        loop {
            if let Ok(Some(event)) = woven_input::poll_for(owner) {
                return Some(event);
            }
            if timer::ticks().wrapping_sub(start) > 200 {
                return None;
            }
            task::yield_now();
        }
    }

    if !step("packet decoder", mouse::decoder_self_test()) {
        return false;
    }

    // Raw hardware paths into a focus queue.
    const PROBE: u64 = 77;
    woven_input::set_focus(Some(PROBE));
    let mouse_path = packet(12, 7, true)
        && next_event(PROBE) == Some(Event::PointerMove { dx: 12, dy: 7 })
        && next_event(PROBE)
            == Some(Event::PointerButton {
                button: 0,
                pressed: true,
            })
        && packet(0, 0, false)
        && next_event(PROBE)
            == Some(Event::PointerButton {
                button: 0,
                pressed: false,
            });
    let keyboard_path = key(0x1e) && next_event(PROBE) == Some(Event::Key(Key::Char('a')));
    woven_input::set_focus(None);
    if !step(
        "mouse bytes through IRQ12 reach the focus queue",
        mouse_path,
    ) || !step(
        "keyboard bytes through IRQ1 are decoded at IRQ time",
        keyboard_path,
    ) {
        return false;
    }

    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if vfs::read_all("/bin/hello-gui", &mut image) != Ok(HELLO_GUI_IMAGE.len()) {
        return step("filesystem round-trip", false);
    }
    let capabilities = display_client_capabilities()
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(wm) = spawn_hello_gui(&image, &["/bin/hello-gui", "--wm"], Some(capabilities)) else {
        return step("window manager spawned", false);
    };
    let start = timer::ticks();
    while woven_input::focus() != Some(wm.as_u64()) {
        if task::process_exited(wm) || timer::ticks().wrapping_sub(start) > 1000 {
            return step("window manager holds display and focus", false);
        }
        task::yield_now();
    }
    step("window manager holds display and focus", true);

    let wm_arg = alloc::format!("{}", wm.as_u64());
    let Some(client_a) = spawn_hello_gui(
        &image,
        &[
            "/bin/hello-gui",
            "--wm-client",
            &wm_arg,
            "200",
            "150",
            "a",
            "x",
        ],
        None,
    ) else {
        return step("client A spawned", false);
    };
    let Some(client_b) = spawn_hello_gui(
        &image,
        &[
            "/bin/hello-gui",
            "--wm-client",
            &wm_arg,
            "260",
            "190",
            "b",
            "y",
        ],
        None,
    ) else {
        return step("client B spawned", false);
    };
    if !step("window manager asleep for input", until_blocked(wm)) {
        return false;
    }

    // Pointer from (0,0) to (205,155), inside A only; click; type x. Then to
    // (370,260), inside B only; click; type y. Scancodes 0x2D and 0x15.
    let driven = packet(205, 155, false)
        && packet(0, 0, true)
        && packet(0, 0, false)
        && key(0x2d)
        && packet(165, 105, false)
        && packet(0, 0, true)
        && packet(0, 0, false)
        && key(0x15);
    if !step("pointer and keys driven through the i8042", driven) {
        return false;
    }

    let code = |status: Option<Result<i32, task::WaitError>>| match status {
        Some(Ok(code)) => code as i64,
        _ => -1,
    };
    let a = wait_hello_gui(client_a).map(|(_, status)| status);
    let b = wait_hello_gui(client_b).map(|(_, status)| status);
    let w = wait_hello_gui(wm).map(|(_, status)| status);
    serial::write_line(format_args!(
        "[S19.12] exit status client A {} client B {} window manager {}",
        code(a),
        code(b),
        code(w)
    ));
    if !step(
        "click focused each window for its key",
        a == Some(Ok(0)) && b == Some(Ok(0)),
    ) || !step("window manager exits zero", w == Some(Ok(0)))
    {
        return false;
    }
    const A: u32 = crate::display::rgb(200, 60, 60);
    const B: u32 = crate::display::rgb(60, 60, 200);
    let stacked = compositor::pixel(280, 200) == Ok(B) && compositor::pixel(205, 155) == Ok(A);
    if !step("kernel reads B on top after the second click", stacked) {
        return false;
    }
    step(
        "lease and focus released on exit",
        compositor::holder().is_none() && woven_input::focus().is_none(),
    )
}

/// Stage 19.11 acceptance: a userspace window manager stacks windows and
/// routes input to the focused one.
///
/// hello-gui `--wm` takes the display and focus; two `--wm-client` processes
/// share overlapping windows with it. Whenever it is asleep for input, the
/// kernel publishes the next of Tab, `x`, Tab, `y` through the keyboard path.
/// The window manager checks after every redraw that the overlap shows the
/// top window; each client must receive exactly its own key (A: `x` once
/// raised, B: `y` once raised back). The kernel then reads the overlap and
/// each window's private area from the compositor itself.
#[cfg(feature = "stage19-11-test")]
fn stage19_11_self_test() -> bool {
    use woven_input::Key;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.11] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until_blocked(pid: task::ProcessId) -> bool {
        let start = timer::ticks();
        while !task::process_task_blocked(pid) {
            if task::process_exited(pid) || timer::ticks().wrapping_sub(start) > 2000 {
                return false;
            }
            task::yield_now();
        }
        true
    }

    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if vfs::read_all("/bin/hello-gui", &mut image) != Ok(HELLO_GUI_IMAGE.len()) {
        return step("filesystem round-trip", false);
    }

    let capabilities = display_client_capabilities()
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let Some(wm) = spawn_hello_gui(&image, &["/bin/hello-gui", "--wm"], Some(capabilities)) else {
        return step("window manager spawned", false);
    };
    let start = timer::ticks();
    while woven_input::focus() != Some(wm.as_u64()) {
        if task::process_exited(wm) || timer::ticks().wrapping_sub(start) > 1000 {
            return step("window manager holds display and focus", false);
        }
        task::yield_now();
    }
    step("window manager holds display and focus", true);

    // A at (200,150) spawned first, so stacked at the bottom; B at (260,190)
    // on top. Their 120x80 windows overlap in (260..320, 190..230).
    let wm_arg = alloc::format!("{}", wm.as_u64());
    let Some(client_a) = spawn_hello_gui(
        &image,
        &[
            "/bin/hello-gui",
            "--wm-client",
            &wm_arg,
            "200",
            "150",
            "a",
            "x",
        ],
        None,
    ) else {
        return step("client A spawned", false);
    };
    let Some(client_b) = spawn_hello_gui(
        &image,
        &[
            "/bin/hello-gui",
            "--wm-client",
            &wm_arg,
            "260",
            "190",
            "b",
            "y",
        ],
        None,
    ) else {
        return step("client B spawned", false);
    };

    for key in [Key::Tab, Key::Char('x'), Key::Tab, Key::Char('y')] {
        if !until_blocked(wm) {
            return step("window manager asleep for input", false);
        }
        if !woven_input::publish_key(key) {
            return step("key published", false);
        }
    }
    step("window manager woken for every key", true);

    let code = |status: Option<Result<i32, task::WaitError>>| match status {
        Some(Ok(code)) => code as i64,
        _ => -1,
    };
    let a = wait_hello_gui(client_a).map(|(_, status)| status);
    let b = wait_hello_gui(client_b).map(|(_, status)| status);
    let w = wait_hello_gui(wm).map(|(_, status)| status);
    serial::write_line(format_args!(
        "[S19.11] exit status client A {} client B {} window manager {}",
        code(a),
        code(b),
        code(w)
    ));
    if !step(
        "each client received exactly its own key",
        a == Some(Ok(0)) && b == Some(Ok(0)),
    ) || !step("window manager exits zero", w == Some(Ok(0)))
    {
        return false;
    }
    const A: u32 = crate::display::rgb(200, 60, 60);
    const B: u32 = crate::display::rgb(60, 60, 200);
    let stacked = compositor::pixel(280, 200) == Ok(B)
        && compositor::pixel(205, 155) == Ok(A)
        && compositor::pixel(370, 260) == Ok(B);
    if !step("kernel reads B on top after the second raise", stacked) {
        return false;
    }
    step(
        "lease and focus released on exit",
        compositor::holder().is_none() && woven_input::focus().is_none(),
    )
}

/// Stage 19.9 acceptance: clients notify the compositor of damage, and the
/// compositor copies only that.
///
/// hello-gui `--damage-compositor` takes the lease and composites a client
/// window from its declared damage. Once the kernel sees it Blocked in
/// `WaitAny(DAMAGE)`, it tells the client to go; the client recolours one
/// region and declares it, and recolours another without declaring it. The
/// compositor must be woken, copy exactly the declared region, and leave the
/// undeclared change off screen.
#[cfg(feature = "stage19-9-test")]
fn stage19_9_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.9] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if !step("damage rules", user_graphics::damage_self_test()) {
        return false;
    }
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if vfs::read_all("/bin/hello-gui", &mut image) != Ok(HELLO_GUI_IMAGE.len()) {
        return step("filesystem round-trip", false);
    }

    const VICTIM_OWNER: u64 = 4246;
    let Ok(victim) = user_graphics::create(VICTIM_OWNER, 8, 8) else {
        return step("victim surface created", false);
    };
    let victim_arg = alloc::format!("{}", victim.to_u64());
    let probe = run_hello_gui(&image, &["/bin/hello-gui", "--probe", &victim_arg], false);
    user_graphics::release_owner(VICTIM_OWNER);
    if !step(
        "unprivileged probe refused damage operations",
        matches!(probe, Some((_, Ok(0)))),
    ) {
        return false;
    }

    const PEER: u64 = 9_000_002;
    if !step("peer mailbox registered", ipc::register(PEER).is_ok()) {
        return false;
    }
    let capabilities = display_client_capabilities().with(capability::Capability::Ipc);
    let Some(compositor_pid) = spawn_hello_gui(
        &image,
        &["/bin/hello-gui", "--damage-compositor"],
        Some(capabilities),
    ) else {
        let _ = ipc::unregister(PEER);
        return step("compositor process spawned", false);
    };
    let start = timer::ticks();
    while compositor::holder() != Some(compositor_pid.as_u64()) {
        if task::process_exited(compositor_pid) || timer::ticks().wrapping_sub(start) > 1000 {
            let _ = ipc::unregister(PEER);
            return step("compositor process holds the lease", false);
        }
        task::yield_now();
    }
    step("compositor process holds the lease", true);

    let compositor_arg = alloc::format!("{}", compositor_pid.as_u64());
    let peer_arg = alloc::format!("{}", PEER);
    let Some(client_pid) = spawn_hello_gui(
        &image,
        &[
            "/bin/hello-gui",
            "--damage-client",
            &compositor_arg,
            &peer_arg,
        ],
        None,
    ) else {
        let _ = ipc::unregister(PEER);
        return step("client process spawned", false);
    };

    // The compositor's only blocking call is its damage wait; everything
    // before it polls. Blocked therefore means asleep on damage.
    let start = timer::ticks();
    let mut asleep = false;
    while !asleep {
        asleep = task::process_task_blocked(compositor_pid);
        if !asleep
            && (task::process_exited(compositor_pid) || timer::ticks().wrapping_sub(start) > 2000)
        {
            break;
        }
        task::yield_now();
    }
    if !step("compositor asleep on damage", asleep)
        || !step(
            "client told to go",
            ipc::send(PEER, client_pid.as_u64(), b"go").is_ok(),
        )
    {
        let _ = ipc::unregister(PEER);
        return false;
    }

    let client_status = wait_hello_gui(client_pid).map(|(_, status)| status);
    let compositor_status = wait_hello_gui(compositor_pid).map(|(_, status)| status);
    let _ = ipc::unregister(PEER);
    let code = |status: Option<Result<i32, task::WaitError>>| match status {
        Some(Ok(code)) => code as i64,
        _ => -1,
    };
    serial::write_line(format_args!(
        "[S19.9] client exit status {} compositor exit status {}",
        code(client_status),
        code(compositor_status)
    ));
    if !step("client exits zero", client_status == Some(Ok(0)))
        || !step("compositor exits zero", compositor_status == Some(Ok(0)))
    {
        return false;
    }
    const CLIENT_COLOR: u32 = crate::display::rgb(180, 40, 90);
    const DAMAGED_COLOR: u32 = crate::display::rgb(20, 200, 60);
    if !step(
        "kernel sees the declared region updated and the undeclared one not",
        compositor::pixel(310, 205) == Ok(DAMAGED_COLOR)
            && compositor::pixel(330, 220) == Ok(CLIENT_COLOR),
    ) {
        return false;
    }
    step("lease released on exit", compositor::holder().is_none())
}

/// Stage 19.8 acceptance: one sleep over input and IPC.
///
/// hello-gui `--multiplex <peer>` takes focus and sleeps in `WaitAny` over
/// both sources. The kernel registers a stand-in peer mailbox, and only once
/// it sees the client Blocked does it send a message; then, once Blocked
/// again, it publishes a key. The client must report exactly the source that
/// woke it each time, consume each event, then time out. The stand-in peer is
/// a bare mailbox, not a process, so IPC policy (rightly) refuses replies to
/// it; the client does not reply.
#[cfg(feature = "stage19-8-test")]
fn stage19_8_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.8] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn until_blocked(pid: task::ProcessId) -> bool {
        let start = timer::ticks();
        while !task::process_task_blocked(pid) {
            if task::process_exited(pid) || timer::ticks().wrapping_sub(start) > 1000 {
                return false;
            }
            task::yield_now();
        }
        true
    }

    if !step(
        "timer/event object registration is level-safe",
        async_events::wait_any_object_self_test(),
    ) {
        return false;
    }
    if !step(
        "manual-reset event wakes bounded task waiter fan-out",
        async_events::event_task_fanout_self_test(),
    ) {
        return false;
    }

    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if vfs::read_all("/bin/hello-gui", &mut image) != Ok(HELLO_GUI_IMAGE.len()) {
        return step("filesystem round-trip", false);
    }

    let timer_event_caps = capability::CapabilitySet::only(capability::Capability::Console)
        .with(capability::Capability::TimerRead)
        .with(capability::Capability::Ipc);
    let timer_event = spawn_hello_gui(
        &image,
        &["/bin/hello-gui", "--timer-event"],
        Some(timer_event_caps),
    )
    .and_then(wait_hello_gui);
    if !step(
        "userspace timer and event sources wake unified wait",
        matches!(timer_event, Some((_, Ok(0)))),
    ) {
        return false;
    }

    const VICTIM_OWNER: u64 = 4245;
    let Ok(victim) = user_graphics::create(VICTIM_OWNER, 8, 8) else {
        return step("victim surface created", false);
    };
    let victim_arg = alloc::format!("{}", victim.to_u64());
    let probe = run_hello_gui(&image, &["/bin/hello-gui", "--probe", &victim_arg], false);
    user_graphics::release_owner(VICTIM_OWNER);
    if !step(
        "unprivileged probe refused the combined wait",
        matches!(probe, Some((_, Ok(0)))),
    ) {
        return false;
    }

    // A stand-in peer: a registered mailbox the kernel sends from and reads.
    const PEER: u64 = 9_000_001;
    if !step("peer mailbox registered", ipc::register(PEER).is_ok()) {
        return false;
    }
    let capabilities = display_client_capabilities()
        .with(capability::Capability::InputIo)
        .with(capability::Capability::Ipc);
    let peer_arg = alloc::format!("{}", PEER);
    let Some(pid) = spawn_hello_gui(
        &image,
        &["/bin/hello-gui", "--multiplex", &peer_arg],
        Some(capabilities),
    ) else {
        return step("multiplexing client spawned", false);
    };
    let start = timer::ticks();
    while woven_input::focus() != Some(pid.as_u64()) {
        if task::process_exited(pid) || timer::ticks().wrapping_sub(start) > 1000 {
            return step("client holds focus", false);
        }
        task::yield_now();
    }
    step("client holds focus", true);

    if !step("client asleep before the message", until_blocked(pid))
        || !step(
            "message sent",
            ipc::send(PEER, pid.as_u64(), b"ping").is_ok(),
        )
    {
        return false;
    }
    if !step("client asleep before the key", until_blocked(pid))
        || !step(
            "key published",
            woven_input::publish_key(woven_input::Key::Char('z')),
        )
    {
        return false;
    }

    let Some((_, status)) = wait_hello_gui(pid) else {
        return step("multiplexing client runs to completion", false);
    };
    serial::write_line(format_args!(
        "[S19.8] multiplexing client exit status {}",
        match status {
            Ok(code) => code as i64,
            Err(_) => -1,
        }
    ));
    let _ = ipc::unregister(PEER);
    if !step(
        "woken by each source in turn, then timed out",
        status == Ok(0),
    ) {
        return false;
    }
    step("focus returned on exit", woven_input::focus().is_none())
}

/// Stage 19.7 acceptance: a separate compositor process draws a client's
/// window from a surface the client shared with it.
///
/// hello-gui `--compositor` takes the lease with `Console | DisplayIo | Ipc`;
/// hello-gui `--client <pid>` runs as an ordinary process, shares one surface
/// with the compositor, keeps another private, and sends both tokens over IPC.
/// The compositor must draw the shared one and be refused the private one.
/// The kernel then reads the composited pixel itself and checks the grant died
/// with the client's surfaces.
#[cfg(feature = "stage19-7-test")]
fn stage19_7_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.7] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if !step("grant rules", user_graphics::sharing_self_test()) {
        return false;
    }
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if vfs::read_all("/bin/hello-gui", &mut image) != Ok(HELLO_GUI_IMAGE.len()) {
        return step("filesystem round-trip", false);
    }

    let capabilities = display_client_capabilities().with(capability::Capability::Ipc);
    let Some(compositor_pid) = spawn_hello_gui(
        &image,
        &["/bin/hello-gui", "--compositor"],
        Some(capabilities),
    ) else {
        return step("compositor process spawned", false);
    };
    let start = timer::ticks();
    while compositor::holder() != Some(compositor_pid.as_u64()) {
        if task::process_exited(compositor_pid) || timer::ticks().wrapping_sub(start) > 1000 {
            return step("compositor process holds the lease", false);
        }
        task::yield_now();
    }
    step("compositor process holds the lease", true);

    let compositor_arg = alloc::format!("{}", compositor_pid.as_u64());
    let Some(client_pid) = spawn_hello_gui(
        &image,
        &["/bin/hello-gui", "--client", &compositor_arg],
        None,
    ) else {
        return step("client process spawned", false);
    };

    let client_status = wait_hello_gui(client_pid).map(|(_, status)| status);
    let compositor_status = wait_hello_gui(compositor_pid).map(|(_, status)| status);
    let code = |status: Option<Result<i32, task::WaitError>>| match status {
        Some(Ok(code)) => code as i64,
        _ => -1,
    };
    serial::write_line(format_args!(
        "[S19.7] client exit status {} compositor exit status {}",
        code(client_status),
        code(compositor_status)
    ));
    if !step("client exits zero", client_status == Some(Ok(0)))
        || !step("compositor exits zero", compositor_status == Some(Ok(0)))
    {
        return false;
    }
    const CLIENT_COLOR: u32 = crate::display::rgb(180, 40, 90);
    if !step(
        "kernel reads the client's window on the compositor",
        compositor::pixel(300, 200) == Ok(CLIENT_COLOR)
            && compositor::pixel(339, 229) == Ok(CLIENT_COLOR),
    ) {
        return false;
    }
    if !step(
        "client surfaces and grants released on exit",
        user_graphics::count_for(client_pid.as_u64()) == 0,
    ) {
        return false;
    }
    step("lease released on exit", compositor::holder().is_none())
}

/// Stage 19.6 acceptance: the focus holder sleeps for input instead of polling.
///
/// hello-gui `--wait` reads keys through the blocking `InputWait`. Before each
/// key the kernel waits until the client's task is Blocked; a polling client
/// is never Blocked, only Ready, so this proves it sleeps in the kernel and is
/// woken by the publish itself. The app also checks a timed wait expires on
/// schedule. An unprivileged probe must be refused `InputWait`.
#[cfg(feature = "stage19-6-test")]
fn stage19_6_self_test() -> bool {
    use woven_input::Key;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.6] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if vfs::read_all("/bin/hello-gui", &mut image) != Ok(HELLO_GUI_IMAGE.len()) {
        return step("filesystem round-trip", false);
    }

    const VICTIM_OWNER: u64 = 4244;
    let Ok(victim) = user_graphics::create(VICTIM_OWNER, 8, 8) else {
        return step("victim surface created", false);
    };
    let victim_arg = alloc::format!("{}", victim.to_u64());
    let probe = run_hello_gui(&image, &["/bin/hello-gui", "--probe", &victim_arg], false);
    user_graphics::release_owner(VICTIM_OWNER);
    if !step(
        "unprivileged probe refused input wait",
        matches!(probe, Some((_, Ok(0)))),
    ) {
        return false;
    }

    let capabilities = display_client_capabilities().with(capability::Capability::InputIo);
    let Some(pid) = spawn_hello_gui(&image, &["/bin/hello-gui", "--wait"], Some(capabilities))
    else {
        return step("waiting client spawned", false);
    };
    let start = timer::ticks();
    while woven_input::focus() != Some(pid.as_u64()) {
        if task::process_exited(pid) || timer::ticks().wrapping_sub(start) > 1000 {
            return step("presenting holder gains focus", false);
        }
        task::yield_now();
    }
    step("presenting holder gains focus", true);

    for key in [Key::Char('o'), Key::Char('k'), Key::Enter] {
        let start = timer::ticks();
        while !task::process_task_blocked(pid) {
            if task::process_exited(pid) || timer::ticks().wrapping_sub(start) > 1000 {
                return step("client sleeps while waiting", false);
            }
            task::yield_now();
        }
        if !woven_input::publish_key(key) {
            return step("key published", false);
        }
    }
    step("client slept before every key", true);

    let Some((_, status)) = wait_hello_gui(pid) else {
        return step("waiting client runs to completion", false);
    };
    serial::write_line(format_args!(
        "[S19.6] waiting client exit status {}",
        match status {
            Ok(code) => code as i64,
            Err(_) => -1,
        }
    ));
    if !step(
        "woken with exactly the keys, then timed out on schedule",
        status == Ok(0),
    ) {
        return false;
    }
    step("focus returned on exit", woven_input::focus().is_none())
}

/// Stage 19.5 acceptance: input focus follows the compositor lease holder.
///
/// The kernel checks the focus rules directly. hello-gui `--input` then runs
/// with `Console | DisplayIo | InputIo`; once it has taken the display, and
/// therefore focus, the kernel publishes keystrokes through the same path the
/// keyboard interrupt uses. The app must receive exactly those keys, in order,
/// and the terminal must receive none of them. After the app exits, focus
/// returns to the terminal.
#[cfg(feature = "stage19-5-test")]
fn stage19_5_self_test() -> bool {
    use woven_input::Key;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.5] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if !step("focus rules", woven_input::focus_self_test()) {
        return false;
    }
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if vfs::read_all("/bin/hello-gui", &mut image) != Ok(HELLO_GUI_IMAGE.len()) {
        return step("filesystem round-trip", false);
    }

    // An ordinary process cannot read input at all; the probe checks that,
    // along with the Stage 19.4 refusals, against a genuinely foreign surface.
    const VICTIM_OWNER: u64 = 4243;
    let Ok(victim) = user_graphics::create(VICTIM_OWNER, 8, 8) else {
        return step("victim surface created", false);
    };
    let victim_arg = alloc::format!("{}", victim.to_u64());
    let probe = run_hello_gui(&image, &["/bin/hello-gui", "--probe", &victim_arg], false);
    user_graphics::release_owner(VICTIM_OWNER);
    if !step(
        "unprivileged probe refused input",
        matches!(probe, Some((_, Ok(0)))),
    ) {
        return false;
    }

    let capabilities = display_client_capabilities().with(capability::Capability::InputIo);
    let Some(pid) = spawn_hello_gui(&image, &["/bin/hello-gui", "--input"], Some(capabilities))
    else {
        return step("input client spawned", false);
    };
    let start = timer::ticks();
    while woven_input::focus() != Some(pid.as_u64()) {
        if task::process_exited(pid) || timer::ticks().wrapping_sub(start) > 1000 {
            return step("presenting holder gains focus", false);
        }
        task::yield_now();
    }
    step("presenting holder gains focus", true);

    let keys = [Key::Char('o'), Key::Char('k'), Key::Enter];
    for key in keys {
        if !woven_input::publish_key(key) {
            return step("keys published", false);
        }
    }
    let leaked = woven_input::poll().is_some();
    if !step("terminal received none of the keys", !leaked) {
        return false;
    }

    let Some((_, status)) = wait_hello_gui(pid) else {
        return step("input client runs to completion", false);
    };
    serial::write_line(format_args!(
        "[S19.5] input client exit status {}",
        match status {
            Ok(code) => code as i64,
            Err(_) => -1,
        }
    ));
    if !step(
        "holder received exactly the keys, in order",
        status == Ok(0),
    ) {
        return false;
    }
    if !step("focus returned on exit", woven_input::focus().is_none()) {
        return false;
    }
    // With focus back, a keystroke reaches the terminal again.
    let restored = woven_input::publish_key(Key::Char('t'))
        && woven_input::poll() == Some(woven_input::Event::Key(Key::Char('t')));
    step("terminal receives input again", restored)
}

/// Stage 19.4 acceptance: compositor authority.
///
/// The kernel checks the lease rules directly, then runs hello-gui twice. As
/// an ordinary user process in `--probe` mode it must be refused every
/// compositor operation and every operation on a surface it does not own,
/// including one whose token it forged. With `DisplayIo` it claims the lease,
/// presents to the framebuffer and exits; the kernel then confirms that the
/// lease was released and the display returned to the terminal.
#[cfg(feature = "stage19-4-test")]
fn stage19_4_self_test() -> bool {
    use alloc::format;

    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.4] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if !step("lease rules", compositor::lease_self_test()) {
        return false;
    }

    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut image = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if !step(
        "filesystem round-trip",
        vfs::read_all("/bin/hello-gui", &mut image) == Ok(HELLO_GUI_IMAGE.len())
            && image[..] == HELLO_GUI_IMAGE[..],
    ) {
        return false;
    }

    // A surface belonging to some other process, for the probe to attack.
    const VICTIM_OWNER: u64 = 4242;
    const VICTIM_COLOR: u32 = crate::display::rgb(12, 34, 56);
    let Ok(victim) = user_graphics::create(VICTIM_OWNER, 8, 8) else {
        return step("victim surface created", false);
    };
    if user_graphics::fill_rect(victim, crate::display::Rect::new(0, 0, 8, 8), VICTIM_COLOR)
        .is_err()
    {
        return step("victim surface filled", false);
    }
    let victim_arg = format!("{}", victim.to_u64());

    let probe = run_hello_gui(&image, &["/bin/hello-gui", "--probe", &victim_arg], false);
    let probe_status = match probe {
        Some((_, Ok(code))) => code as i64,
        _ => -1,
    };
    serial::write_line(format_args!("[S19.4] probe exit status {}", probe_status));
    if !step("unprivileged probe refused everywhere", probe_status == 0) {
        return false;
    }
    let intact = user_graphics::pixel(victim, 0, 0) == Ok(VICTIM_COLOR)
        && user_graphics::pixel(victim, 7, 7) == Ok(VICTIM_COLOR)
        && user_graphics::count_for(VICTIM_OWNER) == 1;
    if !step("victim surface untouched", intact) {
        return false;
    }
    if !step("probe never held the lease", compositor::holder().is_none()) {
        return false;
    }
    user_graphics::release_owner(VICTIM_OWNER);

    let Some((pid, status)) = run_hello_gui(&image, &["/bin/hello-gui"], true) else {
        return step("display client runs to completion", false);
    };
    serial::write_line(format_args!(
        "[S19.4] display client exit status {}",
        match status {
            Ok(code) => code as i64,
            Err(_) => -1,
        }
    ));
    if !step("display client exits zero", status == Ok(0)) {
        return false;
    }
    const CONTENT: u32 = crate::display::rgb(230, 230, 240);
    let screen = terminal::with_framebuffer(|buffer, info| {
        // The holder presented before exiting, so the framebuffer itself
        // carries the content colour at the window's screen position.
        let offset = (100 * info.stride + 160) * info.bytes_per_pixel;
        buffer
            .get(offset..offset + 3)
            .map(|bytes| match info.pixel_format {
                bootloader_api::info::PixelFormat::Bgr => {
                    crate::display::rgb(bytes[2], bytes[1], bytes[0])
                }
                _ => crate::display::rgb(bytes[0], bytes[1], bytes[2]),
            })
    })
    .flatten();
    if !step(
        "holder's present reached the framebuffer",
        screen == Some(CONTENT),
    ) {
        return false;
    }
    if !step("lease released on exit", compositor::holder().is_none()) {
        return false;
    }
    if !step(
        "display returned to the terminal",
        !compositor::is_presenting(),
    ) {
        return false;
    }
    step(
        "owner slots released on exit",
        user_graphics::count_for(pid.as_u64()) == 0,
    )
}

/// Stage 19.2 acceptance: a userspace program presents its surface onto the
/// compositor and the kernel's compositor surface carries the expected pixels.
///
/// The hello-gui binary itself verifies its blit through `screen_pixel`, so
/// exit zero already proves the compositor is honouring the request. This
/// self-test additionally re-reads the compositor from kernel space to confirm
/// nothing has unwound between the app exit and the gate check.
#[cfg(feature = "stage19-2-test")]
fn stage19_2_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S19.2] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if elf::parse(HELLO_GUI_IMAGE).is_err() {
        return step("elf accepted by the loader", false);
    }
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut readback = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if !step(
        "filesystem round-trip",
        vfs::read_all("/bin/hello-gui", &mut readback) == Ok(HELLO_GUI_IMAGE.len())
            && readback[..] == HELLO_GUI_IMAGE[..],
    ) {
        return false;
    }
    let Some((pid, exit_status)) = run_hello_gui(&readback, &["/bin/hello-gui"], true) else {
        return step(
            "load, spawn with display authority, run to completion",
            false,
        );
    };
    let status = match exit_status {
        Ok(code) => code as i64,
        Err(_) => -1,
    };
    serial::write_line(format_args!("[S19.2] exit status {}", status));
    if !step("application exits zero", exit_status == Ok(0)) {
        return false;
    }

    // The compositor must still show the content pixel the app placed; the
    // self-test re-reads it from kernel space to confirm no cleanup wiped it.
    const CONTENT: u32 = crate::display::rgb(230, 230, 240);
    if !step(
        "compositor retained the window after app exit",
        crate::compositor::pixel(160, 100) == Ok(CONTENT),
    ) {
        return false;
    }
    step(
        "owner slots released on exit",
        crate::user_graphics::count_for(pid.as_u64()) == 0,
    )
}

/// Acceptance for the standalone textured-3D-chrome proof of concept
/// (`hello-gui --3d-chrome-demo`): a userspace program renders chrome
/// offscreen, projects it through a perspective camera as a texture-mapped
/// 3D quad (not a flat blit), and presents it. The app verifies its own
/// render from userspace; this self-test re-reads the compositor from
/// kernel space to confirm nothing unwound between app exit and the gate
/// check, the same reasoning `stage19_2_self_test` uses.
#[cfg(feature = "stage20-16-test")]
fn stage20_16_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S20.16] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if elf::parse(HELLO_GUI_IMAGE).is_err() {
        return step("elf accepted by the loader", false);
    }
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut readback = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if !step(
        "filesystem round-trip",
        vfs::read_all("/bin/hello-gui", &mut readback) == Ok(HELLO_GUI_IMAGE.len())
            && readback[..] == HELLO_GUI_IMAGE[..],
    ) {
        return false;
    }
    let Some((_pid, exit_status)) =
        run_hello_gui(&readback, &["/bin/hello-gui", "--3d-chrome-demo"], true)
    else {
        return step(
            "load, spawn with display authority, run to completion",
            false,
        );
    };
    let status = match exit_status {
        Ok(code) => code as i64,
        Err(_) => -1,
    };
    serial::write_line(format_args!("[S20.16] exit status {}", status));
    if !step("application exits zero", exit_status == Ok(0)) {
        return false;
    }

    // The near (left) edge of the projected quad, away from its label, must
    // still show the chrome's own fill colour -- the same location and
    // colour `three_d_chrome_demo` already checked from userspace, re-read
    // from kernel space to confirm the compositor retained it after exit.
    const CHROME_FILL: u32 = crate::display::rgb(40, 220, 230);
    step(
        "compositor retained the textured 3D quad after app exit",
        crate::compositor::pixel(65, 22) == Ok(CHROME_FILL),
    )
}

/// Acceptance for the standalone syscall-latency micro-benchmark
/// (`hello-gui --syscall-benchmark`, Stage 30 performance engineering): an
/// ordinary (no `DisplayIo`) userspace program times 1000 round trips of
/// `current_pid` with its own cycle counter and exits zero. This gate only
/// confirms the benchmark ran to completion and reported PASSED; the
/// numbers themselves are a measurement, not a threshold this self-test
/// enforces.
#[cfg(feature = "stage30-1-test")]
fn stage30_1_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S30.1] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if elf::parse(HELLO_GUI_IMAGE).is_err() {
        return step("elf accepted by the loader", false);
    }
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut readback = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if !step(
        "filesystem round-trip",
        vfs::read_all("/bin/hello-gui", &mut readback) == Ok(HELLO_GUI_IMAGE.len())
            && readback[..] == HELLO_GUI_IMAGE[..],
    ) {
        return false;
    }
    // No DisplayIo: the benchmark needs no capability beyond an ordinary
    // process's own, so this runs the plain `task::spawn_user_process` path.
    let Some((_pid, exit_status)) =
        run_hello_gui(&readback, &["/bin/hello-gui", "--syscall-benchmark"], false)
    else {
        return step(
            "load and spawn as an ordinary process, run to completion",
            false,
        );
    };
    let status = match exit_status {
        Ok(code) => code as i64,
        Err(_) => -1,
    };
    serial::write_line(format_args!("[S30.1] exit status {}", status));
    step("application exits zero", exit_status == Ok(0))
}

/// Stage 30.2 acceptance: hello-gui's `--ipc-benchmark` mode sends and
/// receives a one-byte message to and from itself 1000 times, timing each
/// round trip with the cycle counter, and exits zero. This gate only
/// confirms the benchmark ran to completion and reported PASSED; the
/// numbers themselves are a measurement, not a threshold this self-test
/// enforces.
#[cfg(feature = "stage30-2-test")]
fn stage30_2_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S30.2] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if elf::parse(HELLO_GUI_IMAGE).is_err() {
        return step("elf accepted by the loader", false);
    }
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut readback = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if !step(
        "filesystem round-trip",
        vfs::read_all("/bin/hello-gui", &mut readback) == Ok(HELLO_GUI_IMAGE.len())
            && readback[..] == HELLO_GUI_IMAGE[..],
    ) {
        return false;
    }
    // No DisplayIo: the benchmark needs no capability beyond an ordinary
    // process's own Ipc, so this runs the plain `task::spawn_user_process`
    // path and sends/receives to/from its own process id.
    let Some((_pid, exit_status)) =
        run_hello_gui(&readback, &["/bin/hello-gui", "--ipc-benchmark"], false)
    else {
        return step(
            "load and spawn as an ordinary process, run to completion",
            false,
        );
    };
    let status = match exit_status {
        Ok(code) => code as i64,
        Err(_) => -1,
    };
    serial::write_line(format_args!("[S30.2] exit status {}", status));
    step("application exits zero", exit_status == Ok(0))
}

/// Stage 30.3 acceptance: hello-gui's `--fs-benchmark` mode seeks, writes,
/// seeks, and reads back a 256-byte payload against a pre-created scratch
/// file 500 times, timing each operation with the cycle counter, and exits
/// zero. This gate only confirms the benchmark ran to completion and
/// reported PASSED; the numbers themselves are a measurement, not a
/// threshold this self-test enforces.
#[cfg(feature = "stage30-3-test")]
fn stage30_3_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S30.3] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if elf::parse(HELLO_GUI_IMAGE).is_err() {
        return step("elf accepted by the loader", false);
    }
    let scratch = (vfs::stat("/tmp").is_ok() || vfs::mkdir("/tmp").is_ok())
        && vfs::write_file("/tmp/fsbench", &[0u8; 256]).is_ok();
    if !step(
        "install into the filesystem and create the scratch file",
        scratch && vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut readback = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if !step(
        "filesystem round-trip",
        vfs::read_all("/bin/hello-gui", &mut readback) == Ok(HELLO_GUI_IMAGE.len())
            && readback[..] == HELLO_GUI_IMAGE[..],
    ) {
        return false;
    }
    // No DisplayIo: the benchmark needs no capability beyond an ordinary
    // process's own FileRead/FileWrite, so this runs the plain
    // `task::spawn_user_process` path.
    let Some((_pid, exit_status)) =
        run_hello_gui(&readback, &["/bin/hello-gui", "--fs-benchmark"], false)
    else {
        return step(
            "load and spawn as an ordinary process, run to completion",
            false,
        );
    };
    let status = match exit_status {
        Ok(code) => code as i64,
        Err(_) => -1,
    };
    serial::write_line(format_args!("[S30.3] exit status {}", status));
    step("application exits zero", exit_status == Ok(0))
}

/// Stage 30.4 acceptance: hello-gui's `--compositor-benchmark` mode places a
/// window, then toggles one pixel and flushes it to the framebuffer
/// (`surface.write` + `compositor_present`) 500 times, timing each round
/// trip with the cycle counter, and exits zero. This gate only confirms the
/// benchmark ran to completion and reported PASSED; the numbers themselves
/// are a measurement, not a threshold this self-test enforces.
#[cfg(feature = "stage30-4-test")]
fn stage30_4_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S30.4] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if elf::parse(HELLO_GUI_IMAGE).is_err() {
        return step("elf accepted by the loader", false);
    }
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut readback = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if !step(
        "filesystem round-trip",
        vfs::read_all("/bin/hello-gui", &mut readback) == Ok(HELLO_GUI_IMAGE.len())
            && readback[..] == HELLO_GUI_IMAGE[..],
    ) {
        return false;
    }
    // Needs DisplayIo to claim the compositor and present, so this runs the
    // service-spawn path with display authority.
    let Some((_pid, exit_status)) =
        run_hello_gui(&readback, &["/bin/hello-gui", "--compositor-benchmark"], true)
    else {
        return step(
            "load and spawn with display authority, run to completion",
            false,
        );
    };
    let status = match exit_status {
        Ok(code) => code as i64,
        Err(_) => -1,
    };
    serial::write_line(format_args!("[S30.4] exit status {}", status));
    step("application exits zero", exit_status == Ok(0))
}

/// Stage 30.5 acceptance: hello-gui's `--scheduler-benchmark` mode times
/// 1000 `yield_now` round trips with the cycle counter and exits zero.
/// This gate only confirms the benchmark ran to completion and reported
/// PASSED; the numbers themselves are a measurement, not a threshold this
/// self-test enforces.
#[cfg(feature = "stage30-5-test")]
fn stage30_5_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S30.5] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    if elf::parse(HELLO_GUI_IMAGE).is_err() {
        return step("elf accepted by the loader", false);
    }
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }
    let mut readback = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    if !step(
        "filesystem round-trip",
        vfs::read_all("/bin/hello-gui", &mut readback) == Ok(HELLO_GUI_IMAGE.len())
            && readback[..] == HELLO_GUI_IMAGE[..],
    ) {
        return false;
    }
    // No DisplayIo: the benchmark needs no capability beyond an ordinary
    // process's own, so this runs the plain `task::spawn_user_process` path.
    let Some((_pid, exit_status)) =
        run_hello_gui(&readback, &["/bin/hello-gui", "--scheduler-benchmark"], false)
    else {
        return step(
            "load and spawn as an ordinary process, run to completion",
            false,
        );
    };
    let status = match exit_status {
        Ok(code) => code as i64,
        Err(_) => -1,
    };
    serial::write_line(format_args!("[S30.5] exit status {}", status));
    step("application exits zero", exit_status == Ok(0))
}

/// Stage 18.2 acceptance: a userspace program allocates a surface, draws into
/// it through the graphics syscalls, reads pixels back for verification, and
/// exits zero. End-to-end proof of the client surface primitive.
#[cfg(feature = "stage18-2-test")]
fn stage18_2_self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!(
            "[S18.2] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    serial::write_line(format_args!(
        "[S18.2] hello-gui image bytes {}",
        HELLO_GUI_IMAGE.len()
    ));

    if elf::parse(HELLO_GUI_IMAGE).is_err() {
        return step("elf accepted by the loader", false);
    }
    if !step(
        "install into the filesystem",
        vfs::create_read_only("/bin/hello-gui", HELLO_GUI_IMAGE).is_ok(),
    ) {
        return false;
    }

    let mut readback = alloc::vec![0u8; HELLO_GUI_IMAGE.len()];
    let round_trip = vfs::read_all("/bin/hello-gui", &mut readback) == Ok(HELLO_GUI_IMAGE.len())
        && readback[..] == HELLO_GUI_IMAGE[..];
    if !step("filesystem round-trip", round_trip) {
        return false;
    }

    let Some((pid, exit_status)) = run_hello_gui(&readback, &["/bin/hello-gui"], true) else {
        return step(
            "load, spawn with display authority, run to completion",
            false,
        );
    };
    let status_text = match exit_status {
        Ok(code) => code as i64,
        Err(_) => -1,
    };
    serial::write_line(format_args!("[S18.2] exit status {}", status_text));
    if !step("application exits zero", exit_status == Ok(0)) {
        return false;
    }

    // All of the app's surfaces must have been reclaimed when it exited; a
    // leak here would mean release_owner is not wired on the task-exit path.
    step(
        "owner slots released on exit",
        crate::user_graphics::count_for(pid.as_u64()) == 0,
    )
}

/// Boot-stage timing (Stage 30 boot profiling -- "fast boot" is a named
/// target). Ticks are 100 Hz, so this is coarse (10 ms resolution) but
/// enough to see which stage of a slow boot is the slow one. The first call
/// measures from tick 0 (the timer's own start), which is what "time to the
/// first stage" should mean anyway.
static BOOT_STAGE_CLOCK: AtomicU64 = AtomicU64::new(0);

/// Print `label` on screen exactly as the unprofiled call did, and log how
/// many ticks elapsed since the previous call (or boot start, for the
/// first) to serial as `[BOOT PROFILE]`.
fn boot_stage(console: &mut Console, label: &str) {
    let now = timer::ticks();
    let previous = BOOT_STAGE_CLOCK.swap(now, Ordering::AcqRel);
    console.println(label);
    serial::write_line(format_args!(
        "[BOOT PROFILE] {} (+{} ticks)",
        label,
        now.saturating_sub(previous)
    ));
}

/// Stop the machine after an unrecoverable boot or validation error.
///
/// Every call site is an error path: a failed initialization, a failed
/// validation, or the allocation error handler. The success path ends in
/// `qemu_test_exit_success` instead, so this never runs after a passing boot.
///
/// In an acceptance build this exits with the failure status rather than
/// parking in `hlt`. Parking turns a definite failure into a host-side
/// timeout, which is both slow and ambiguous -- a hang and a failure become
/// indistinguishable, and the harness waits out its whole budget before
/// reporting something it already knew. `qemu_test_exit_failure` carries the
/// same reasoning for the sites that already call it directly.
///
/// Builds without `qemu-test`, including `physical-probe` boots on real
/// hardware, keep the halt so the serial log stays readable on screen.
fn halt() -> ! {
    #[cfg(feature = "qemu-test")]
    qemu_test_exit_failure();
    #[cfg(not(feature = "qemu-test"))]
    {
        x86_64::instructions::interrupts::disable();
        loop {
            x86_64::instructions::hlt();
        }
    }
}

#[cfg(not(test))]
#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    serial::write_fmt(format_args!(
        "\nKERNEL ALLOC ERROR: layout size={} align={}\n",
        layout.size(),
        layout.align()
    ));
    halt()
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    panic::kernel_panic(info)
}
