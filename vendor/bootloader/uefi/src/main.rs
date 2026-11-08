#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]

use crate::memory_descriptor::UefiMemoryDescriptor;
use bootloader_api::info::FrameBufferInfo;
use bootloader_boot_config::BootConfig;
use bootloader_x86_64_common::{
    legacy_memory_region::LegacyFrameAllocator, Kernel, RawFrameBufferInfo, SystemInfo,
};
use core::net::Ipv4Addr;
use core::{ptr, slice};
use uefi::mem::memory_map::{MemoryMap, MemoryMapMut};
use uefi::table::cfg::ConfigTableEntry;
use uefi::{
    boot,
    boot::{AllocateType, MemoryType},
    cstr16, cstr8,
    prelude::{entry, Status},
    proto::{
        console::gop::{GraphicsOutput, PixelFormat},
        device_path::DevicePath,
        loaded_image::LoadedImage,
        media::file::{File, FileAttribute, FileInfo, FileMode},
        network::pxe::{BaseCode, DhcpV4Packet},
        ProtocolPointer,
    },
    CStr16, CStr8,
};
use x86_64::{
    structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

mod config;
mod image;
mod memory_descriptor;

#[derive(Clone, Copy)]
enum FileRole {
    Kernel,
    Configuration,
    Ramdisk,
}

struct BootFile {
    disk: &'static CStr16,
    tftp: &'static CStr8,
    role: FileRole,
}

const KERNEL_FILE: BootFile = BootFile {
    disk: cstr16!("kernel-x86_64"),
    tftp: cstr8!("kernel-x86_64"),
    role: FileRole::Kernel,
};
const RECOVERY_KERNEL_FILE: BootFile = BootFile {
    disk: cstr16!("kernel-recovery"),
    tftp: cstr8!("kernel-recovery"),
    role: FileRole::Kernel,
};
const DIAGNOSTIC_KERNEL_FILE: BootFile = BootFile {
    disk: cstr16!("kernel-diagnostic"),
    tftp: cstr8!("kernel-diagnostic"),
    role: FileRole::Kernel,
};
const CONFIG_FILE: BootFile = BootFile {
    disk: cstr16!("boot.json"),
    tftp: cstr8!("boot.json"),
    role: FileRole::Configuration,
};
const RAMDISK_FILE: BootFile = BootFile {
    disk: cstr16!("ramdisk"),
    tftp: cstr8!("ramdisk"),
    role: FileRole::Ramdisk,
};

#[entry]
fn main() -> Status {
    let mut boot_mode = BootMode::Disk;
    let config_file = load_config_file(BootMode::Disk);
    let mut error_loading_config = None;
    let policy = match config_file.as_deref().map(config::parse).transpose() {
        Ok(data) => data.unwrap_or_else(default_policy),
        Err(err) => {
            error_loading_config = Some(err);
            default_policy()
        }
    };
    let kernel_file = kernel_file_for(policy.handoff.entry);

    let mut kernel = load_kernel(kernel_file, boot_mode);
    if kernel.is_none() {
        // Try TFTP boot
        boot_mode = BootMode::Tftp;
        kernel = load_kernel(kernel_file, boot_mode);
    }
    let kernel = kernel.unwrap_or_else(|| reject_kernel("Missing"));

    let mut config = policy.boot;

    #[allow(deprecated)]
    if config.frame_buffer.minimum_framebuffer_height.is_none() {
        config.frame_buffer.minimum_framebuffer_height =
            kernel.config.frame_buffer.minimum_framebuffer_height;
    }
    #[allow(deprecated)]
    if config.frame_buffer.minimum_framebuffer_width.is_none() {
        config.frame_buffer.minimum_framebuffer_width =
            kernel.config.frame_buffer.minimum_framebuffer_width;
    }
    let framebuffer = init_logger(&config);

    log::info!("UEFI bootloader started");

    if let Some(framebuffer) = framebuffer {
        log::info!("Using framebuffer at {:#x}", framebuffer.addr);
    }

    if let Some(err) = error_loading_config {
        // Defaults above are used only to establish a diagnostic console.
        // Invalid policy must never silently continue to kernel handoff.
        reject_configuration(err);
    } else {
        log::info!("Reading configuration from disk was successful");
    }

    log::info!("Trying to load ramdisk via {:?}", boot_mode);
    // Ramdisk must load from same source, or not at all.
    let ramdisk = load_ramdisk(boot_mode);

    log::info!(
        "{}",
        match ramdisk {
            Some(_) => "Loaded ramdisk",
            None => "Ramdisk not found.",
        }
    );

    log::trace!("exiting boot services");
    let mut memory_map = unsafe { boot::exit_boot_services(None) };

    memory_map.sort();

    let mut frame_allocator =
        LegacyFrameAllocator::new(memory_map.entries().copied().map(UefiMemoryDescriptor));

    let max_phys_addr = frame_allocator.max_phys_addr();
    let page_tables = create_page_tables(&mut frame_allocator, max_phys_addr, framebuffer.as_ref());
    let mut ramdisk_len = 0u64;
    let ramdisk_addr = if let Some(rd) = ramdisk {
        ramdisk_len = rd.len() as u64;
        Some(rd.as_ptr() as usize as u64)
    } else {
        None
    };
    let system_info = SystemInfo {
        framebuffer,
        rsdp_addr: {
            uefi::system::with_config_table(|config_entries| {
                // look for an ACPI2 RSDP first
                let acpi2_rsdp = config_entries
                    .iter()
                    .find(|entry| matches!(entry.guid, ConfigTableEntry::ACPI2_GUID));
                // if no ACPI2 RSDP is found, look for a ACPI1 RSDP
                let rsdp = acpi2_rsdp.or_else(|| {
                    config_entries
                        .iter()
                        .find(|entry| matches!(entry.guid, ConfigTableEntry::ACPI_GUID))
                });
                rsdp.map(|entry| PhysAddr::new(entry.address as u64))
            })
        },
        ramdisk_addr,
        ramdisk_len,
        boot_policy: policy.handoff,
    };

    bootloader_x86_64_common::load_and_switch_to_kernel(
        kernel,
        config,
        frame_allocator,
        page_tables,
        system_info,
    );
}

#[derive(Clone, Copy, Debug)]
pub enum BootMode {
    Disk,
    Tftp,
}

fn default_policy() -> config::Policy {
    config::Policy {
        boot: BootConfig::default(),
        handoff: bootloader_api::info::BootPolicy::default(),
    }
}

fn kernel_file_for(entry: u8) -> &'static BootFile {
    match entry {
        0 => &KERNEL_FILE,
        1 => &RECOVERY_KERNEL_FILE,
        2 => &DIAGNOSTIC_KERNEL_FILE,
        _ => reject_configuration(config::Error::UnsupportedPolicy),
    }
}

fn reject_configuration(error: config::Error) -> ! {
    log::error!("[WBOOT] invalid boot configuration: {:?}", error);
    // This also reports size failures before the framebuffer logger exists:
    // the existing panic handler writes to the firmware console while boot
    // services are still available. Never include untrusted policy contents.
    panic!(
        "[WBOOT] invalid boot configuration: {:?}\n[WBOOT] boot stopped: invalid boot configuration",
        error
    );
}

fn reject_kernel(error: &str) -> ! {
    log::error!("[WBOOT] invalid kernel image: {}", error);
    panic!(
        "[WBOOT] invalid kernel image: {}\n[WBOOT] boot stopped: invalid kernel image",
        error
    );
}

fn reject_firmware_resource(error: &str) -> ! {
    log::error!("[WBOOT] firmware resource failure: {}", error);
    panic!(
        "[WBOOT] firmware resource failure: {}\n[WBOOT] boot stopped: firmware resource failure",
        error
    );
}

/// Fail closed after `ExitBootServices`. Firmware recovery is no longer legal,
/// but the serial/framebuffer logger remains available for an exact diagnosis.
fn reject_post_exit_resource(error: &str) -> ! {
    log::error!("[WBOOT] post-exit resource failure: {}", error);
    panic!(
        "[WBOOT] post-exit resource failure: {}\n[WBOOT] boot stopped: post-exit resource failure",
        error
    );
}

fn check_file_size(size: usize, role: FileRole) {
    match role {
        FileRole::Configuration => {
            if let Err(error) = config::validate_size(size) {
                reject_configuration(error);
            }
        }
        FileRole::Kernel => {
            if let Err(error) = image::validate_size(size) {
                reject_kernel(match error {
                    image::Error::Empty => "Empty",
                    image::Error::TooLarge => "TooLarge",
                    _ => unreachable!(),
                });
            }
        }
        FileRole::Ramdisk => {}
    }
}

fn load_ramdisk(boot_mode: BootMode) -> Option<&'static mut [u8]> {
    load_file_from_boot_method(&RAMDISK_FILE, boot_mode)
}

fn load_config_file(boot_mode: BootMode) -> Option<&'static mut [u8]> {
    load_file_from_boot_method(&CONFIG_FILE, boot_mode)
}

fn load_kernel(file: &BootFile, boot_mode: BootMode) -> Option<Kernel<'static>> {
    let kernel_slice = load_file_from_boot_method(file, boot_mode)?;
    let config = image::validate(kernel_slice).unwrap_or_else(|error| {
        log::error!("[WBOOT] kernel validation detail: {:?}", error);
        reject_kernel("InvalidElfOrContract")
    });
    let elf = xmas_elf::ElfFile::new(kernel_slice)
        .unwrap_or_else(|_| reject_kernel("InvalidElfOrContract"));
    Some(Kernel {
        elf,
        config,
        start_address: kernel_slice.as_ptr(),
        len: kernel_slice.len(),
    })
}

fn load_file_from_boot_method(
    filename: &BootFile,
    boot_mode: BootMode,
) -> Option<&'static mut [u8]> {
    match boot_mode {
        BootMode::Disk => load_file_from_disk(filename.disk, filename.role),
        BootMode::Tftp => load_file_from_tftp_boot_server(filename.tftp, filename.role),
    }
}

fn load_file_from_disk(name: &CStr16, role: FileRole) -> Option<&'static mut [u8]> {
    // A loader entered through a PXE device has no filesystem protocol on its
    // loaded-image handle. That is the normal disk-unavailable condition which
    // selects the existing disk-to-TFTP fallback. Once a filesystem is found,
    // all open/metadata/read failures remain explicit firmware-resource
    // failures below.
    let Ok(mut file_system) = boot::get_image_file_system(boot::image_handle()) else {
        return None;
    };

    let mut root = file_system
        .open_volume()
        .unwrap_or_else(|_| reject_firmware_resource("DiskOpenVolume"));
    let file_handle_result = root.open(name, FileMode::Read, FileAttribute::empty());
    let file_handle = file_handle_result.ok()?;

    let mut file = match file_handle
        .into_type()
        .unwrap_or_else(|_| reject_firmware_resource("DiskFileType"))
    {
        uefi::proto::media::file::FileType::Regular(f) => f,
        uefi::proto::media::file::FileType::Dir(_) => reject_firmware_resource("DiskFileIsDirectory"),
    };

    let mut buf = [0; 500];
    let file_info: &mut FileInfo = file
        .get_info(&mut buf)
        .unwrap_or_else(|_| reject_firmware_resource("DiskFileInfo"));
    let file_size = usize::try_from(file_info.file_size())
        .unwrap_or_else(|_| reject_firmware_resource("DiskFileSize"));
    check_file_size(file_size, role);

    let file_slice =
        allocate_loader_data(file_size).unwrap_or_else(|error| reject_firmware_resource(error));
    let bytes_read = file
        .read(file_slice)
        .unwrap_or_else(|_| reject_firmware_resource("DiskRead"));
    if bytes_read != file_size {
        reject_firmware_resource("DiskShortRead");
    }

    Some(file_slice)
}

/// Try to load a kernel from a TFTP boot server.
fn load_file_from_tftp_boot_server(name: &CStr8, role: FileRole) -> Option<&'static mut [u8]> {
    let mut base_code = open_pxe_base_code()?;

    // Find the TFTP boot server.
    let mode = base_code.mode();
    let dhcpv4: &DhcpV4Packet = mode.dhcp_ack().as_ref();
    let server_ip = Ipv4Addr::from_octets(dhcpv4.bootp_si_addr);

    // Determine the kernel file size.
    // Once PXE has supplied a DHCP boot server, a TFTP size failure is an I/O
    // failure, not the normal absence used to select the disk-to-TFTP
    // fallback. Do not continue with a partially established network source.
    let file_size = base_code
        .tftp_get_file_size(&server_ip.into(), name)
        .unwrap_or_else(|_| reject_firmware_resource("TftpFileSize"));
    let kernel_size = usize::try_from(file_size)
        .unwrap_or_else(|_| reject_firmware_resource("TftpFileSize"));
    check_file_size(kernel_size, role);

    // Allocate some memory for the kernel file.
    let slice =
        allocate_loader_data(kernel_size).unwrap_or_else(|error| reject_firmware_resource(error));

    // Load the kernel file.
    base_code
        .tftp_read_file(&server_ip.into(), name, Some(slice))
        .unwrap_or_else(|_| reject_firmware_resource("TftpRead"));

    Some(slice)
}

fn allocate_loader_data(size: usize) -> Result<&'static mut [u8], &'static str> {
    if size == 0 {
        return Err("EmptyLoaderData");
    }
    let ptr = boot::allocate_pages(
        AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        size.checked_add(4095).ok_or("LoaderSizeOverflow")? / 4096,
    )
    .map_err(|_| "LoaderAllocation")?;
    unsafe { ptr::write_bytes(ptr.as_ptr(), 0, size) };
    Ok(unsafe { slice::from_raw_parts_mut(ptr.as_ptr(), size) })
}

/// Opens PXE Base Code for the device path that loaded this image.
///
/// Firmware may expose multiple PXE handles. Resolving through the loaded
/// image's device path keeps TFTP reads tied to the booting network device.
fn open_pxe_base_code() -> Option<boot::ScopedProtocol<BaseCode>> {
    let base_code = locate_and_open_protocol_from_image_device_path::<BaseCode>()?;
    base_code.mode().dhcp_ack_received().then_some(base_code)
}

fn locate_and_open_protocol_from_image_device_path<P: ProtocolPointer + ?Sized>(
) -> Option<boot::ScopedProtocol<P>> {
    let image_handle = boot::image_handle();
    let loaded_image = boot::open_protocol_exclusive::<LoadedImage>(image_handle).ok()?;
    let device_handle = loaded_image.device()?;
    let device_path = boot::open_protocol_exclusive::<DevicePath>(device_handle).ok()?;
    let handle = boot::locate_device_path::<P>(&mut &*device_path).ok()?;
    boot::open_protocol_exclusive::<P>(handle).ok()
}

/// Creates page table abstraction types for both the bootloader and kernel page tables.
fn create_page_tables(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    max_phys_addr: PhysAddr,
    frame_buffer: Option<&RawFrameBufferInfo>,
) -> bootloader_x86_64_common::PageTables {
    // UEFI identity-maps all memory, so the offset between physical and virtual addresses is 0
    let phys_offset = VirtAddr::new(0);

    // copy the currently active level 4 page table, because it might be read-only
    log::trace!("switching to new level 4 table");
    let bootloader_page_table = {
        let old_table = {
            let frame = x86_64::registers::control::Cr3::read().0;
            let ptr: *const PageTable = (phys_offset + frame.start_address().as_u64()).as_ptr();
            unsafe { &*ptr }
        };
        let new_frame = frame_allocator
            .allocate_frame()
            .unwrap_or_else(|| reject_post_exit_resource("PageTableFrameExhausted"));
        let new_table: &mut PageTable = {
            let ptr: *mut PageTable =
                (phys_offset + new_frame.start_address().as_u64()).as_mut_ptr();
            // create a new, empty page table
            unsafe {
                ptr.write(PageTable::new());
                &mut *ptr
            }
        };

        // copy the pml4 entries for all identity mapped memory.
        let end_addr = VirtAddr::new(max_phys_addr.as_u64() - 1);
        for p4 in 0..=usize::from(end_addr.p4_index()) {
            new_table[p4] = old_table[p4].clone();
        }

        // copy the pml4 entry for the frame buffer (the frame buffer is not
        // necessarily part of the identity mapping).
        if let Some(frame_buffer) = frame_buffer {
            let start_addr = VirtAddr::new(frame_buffer.addr.as_u64());
            let end_addr = start_addr + frame_buffer.info.byte_len as u64;
            for p4 in usize::from(start_addr.p4_index())..=usize::from(end_addr.p4_index()) {
                new_table[p4] = old_table[p4].clone();
            }
        }

        // the first level 4 table entry is now identical, so we can just load the new one
        unsafe {
            x86_64::registers::control::Cr3::write(
                new_frame,
                x86_64::registers::control::Cr3Flags::empty(),
            );
            OffsetPageTable::new(&mut *new_table, phys_offset)
        }
    };

    // create a new page table hierarchy for the kernel
    let (kernel_page_table, kernel_level_4_frame) = {
        // get an unused frame for new level 4 page table
        let frame: PhysFrame = frame_allocator
            .allocate_frame()
            .unwrap_or_else(|| reject_post_exit_resource("PageTableFrameExhausted"));
        log::info!("New page table at: {:#?}", frame);
        // get the corresponding virtual address
        let addr = phys_offset + frame.start_address().as_u64();
        // initialize a new page table
        let ptr = addr.as_mut_ptr();
        unsafe { *ptr = PageTable::new() };
        let level_4_table = unsafe { &mut *ptr };
        (
            unsafe { OffsetPageTable::new(level_4_table, phys_offset) },
            frame,
        )
    };

    bootloader_x86_64_common::PageTables {
        bootloader: bootloader_page_table,
        kernel: kernel_page_table,
        kernel_level_4_frame,
    }
}

fn init_logger(config: &BootConfig) -> Option<RawFrameBufferInfo> {
    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>().ok()?;
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle).ok()?;

    let mode = {
        let modes = gop.modes();
        match (
            config
                .frame_buffer
                .minimum_framebuffer_height
                .map(|v| {
                    usize::try_from(v)
                        .unwrap_or_else(|_| reject_firmware_resource("GraphicsHeight"))
                }),
            config
                .frame_buffer
                .minimum_framebuffer_width
                .map(|v| {
                    usize::try_from(v)
                        .unwrap_or_else(|_| reject_firmware_resource("GraphicsWidth"))
                }),
        ) {
            (Some(height), Some(width)) => modes
                .filter(|m| {
                    let res = m.info().resolution();
                    res.1 >= height && res.0 >= width
                })
                .last(),
            (Some(height), None) => modes.filter(|m| m.info().resolution().1 >= height).last(),
            (None, Some(width)) => modes.filter(|m| m.info().resolution().0 >= width).last(),
            _ => None,
        }
    };
    let graphics_mode_was_requested = config.frame_buffer.minimum_framebuffer_height.is_some()
        || config.frame_buffer.minimum_framebuffer_width.is_some();
    if graphics_mode_was_requested && mode.is_none() {
        reject_firmware_resource("GraphicsUnsupportedMode");
    }
    if let Some(mode) = mode {
        gop.set_mode(&mode)
            .unwrap_or_else(|_| reject_firmware_resource("GraphicsSetMode"));
    }

    let mode_info = gop.current_mode_info();
    let mut framebuffer = gop.frame_buffer();
    let slice = unsafe { slice::from_raw_parts_mut(framebuffer.as_mut_ptr(), framebuffer.size()) };
    let info = FrameBufferInfo {
        byte_len: framebuffer.size(),
        width: mode_info.resolution().0,
        height: mode_info.resolution().1,
        pixel_format: match mode_info.pixel_format() {
            PixelFormat::Rgb => bootloader_api::info::PixelFormat::Rgb,
            PixelFormat::Bgr => bootloader_api::info::PixelFormat::Bgr,
            PixelFormat::Bitmask | PixelFormat::BltOnly => {
                reject_firmware_resource("GraphicsPixelFormat")
            }
        },
        bytes_per_pixel: 4,
        stride: mode_info.stride(),
    };

    bootloader_x86_64_common::init_logger(
        slice,
        info,
        config.log_level,
        config.frame_buffer_logging,
        config.serial_logging,
    );

    Some(RawFrameBufferInfo {
        addr: PhysAddr::new(framebuffer.as_mut_ptr() as u64),
        info,
    })
}

#[cfg(target_os = "uefi")]
fn uefi_stdout_available() -> bool {
    uefi::table::system_table_raw()
        .map(|st| {
            // SAFETY: The UEFI entry macro sets this pointer before calling
            // `main`, and `system_table_raw` only returns a non-null pointer.
            let st = unsafe { st.as_ref() };
            !st.boot_services.is_null() && !st.stdout.is_null()
        })
        .unwrap_or(false)
}

#[cfg(target_os = "uefi")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use core::arch::asm;
    use core::fmt::Write;

    if uefi_stdout_available() {
        uefi::system::with_stdout(|stdout| {
            let _ = stdout.clear();
            let _ = writeln!(stdout, "{}", info);
        });
    }

    unsafe {
        bootloader_x86_64_common::logger::LOGGER
            .get()
            .map(|l| l.force_unlock())
    };
    log::error!("{}", info);

    loop {
        unsafe { asm!("cli; hlt") };
    }
}
