//! Bounded UEFI exit transaction. No allocation, protocol, or console call is
//! allowed after the first exit attempt; even failure can partially stop firmware.

use core::sync::atomic::{AtomicBool, Ordering};
use uefi::boot::{self, MemoryType};
use uefi::mem::memory_map::{MemoryMap, MemoryMapMeta, MemoryMapRefMut};
use uefi::Status;

pub static ATTEMPTED: AtomicBool = AtomicBool::new(false);

pub fn run() -> MemoryMapRefMut<'static> {
    let initial = boot::memory_map(MemoryType::LOADER_DATA)
        .unwrap_or_else(|_| crate::reject_firmware_resource("MemoryMapAllocation"));
    let meta = initial.meta();
    let size = meta
        .desc_size
        .checked_mul(64)
        .and_then(|slack| meta.map_size.checked_add(slack))
        .filter(|size| *size <= 1024 * 1024)
        .unwrap_or_else(|| crate::reject_firmware_resource("MemoryMapSize"));
    drop(initial);
    let buffer = crate::allocate_loader_data(size)
        .unwrap_or_else(|_| crate::reject_firmware_resource("MemoryMapAllocation"));
    let table = uefi::table::system_table_raw()
        .unwrap_or_else(|| crate::reject_firmware_resource("SystemTable"));
    // SAFETY: UEFI entry supplied this table; all protocol handles have been
    // dropped. Capture only raw function pointers and the loader handle before
    // the first exit. Firmware memory is not reclaimed during this function.
    let services = unsafe { &*table.as_ref().boot_services };
    let get_map = services.get_memory_map;
    let exit = services.exit_boot_services;
    let handle = boot::image_handle().as_ptr();
    for attempt in 0..2 {
        let mut map_size = buffer.len();
        let mut key = 0;
        let mut desc_size = 0;
        let mut desc_version = 0;
        // SAFETY: page-aligned, live loader-owned buffer and initialized scalar
        // out-pointers. GetMemoryMap is legal both before and after failed exit.
        let status = unsafe {
            get_map(
                &mut map_size,
                buffer.as_mut_ptr().cast(),
                &mut key,
                &mut desc_size,
                &mut desc_version,
            )
        };
        if status != Status::SUCCESS {
            if ATTEMPTED.load(Ordering::Acquire) {
                crate::reject_post_exit_resource("MemoryMapRead");
            }
            crate::reject_firmware_resource("MemoryMapRead");
        }
        let valid_map = map_size != 0
            && map_size <= buffer.len()
            && desc_size >= core::mem::size_of::<boot::MemoryDescriptor>()
            && desc_size.is_multiple_of(core::mem::align_of::<boot::MemoryDescriptor>())
            && map_size.is_multiple_of(desc_size);
        if !valid_map {
            if ATTEMPTED.load(Ordering::Acquire) {
                crate::reject_post_exit_resource("MemoryMapInvalid");
            }
            crate::reject_firmware_resource("MemoryMapInvalid");
        }
        #[cfg(feature = "qualification-exit-retry")]
        if attempt == 0 {
            // Allocate once BEFORE the first exit attempt to exercise firmware's
            // real stale-key rejection, then reacquire the map on retry.
            let _reserved =
                boot::allocate_pages(boot::AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
                    .unwrap_or_else(|_| crate::reject_firmware_resource("ExitFixtureAllocation"));
        }
        #[cfg(feature = "qualification-exit-failure")]
        let key = key.wrapping_add(1);
        ATTEMPTED.store(true, Ordering::Release);
        // SAFETY: current map key, original loader handle, no allocation or
        // protocol/console operations between map acquisition and this call.
        let status = unsafe { exit(handle, key) };
        if status == Status::SUCCESS {
            let map = MemoryMapRefMut::new(
                buffer,
                MemoryMapMeta {
                    map_size,
                    desc_size,
                    desc_version,
                    // The opaque key has served its sole purpose at ExitBootServices.
                    // This view is only sorted/iterated, never passed to firmware.
                    map_key: Default::default(),
                },
            )
            .unwrap_or_else(|_| crate::reject_post_exit_resource("MemoryMapInvalid"));
            log::info!(
                "[WBOOT] ExitBootServices succeeded attempts={}",
                attempt + 1
            );
            return map;
        }
        if status != Status::INVALID_PARAMETER || attempt == 1 {
            crate::reject_post_exit_resource("ExitBootServices");
        }
    }
    unreachable!()
}
