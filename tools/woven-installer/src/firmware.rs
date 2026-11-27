extern crate alloc;
use alloc::{format, string::String, vec::Vec};
use core::fmt::Write;
use uefi::{Identify, Result};
use uefi::{
    boot::{self, SearchType},
    prelude::*,
    proto::{
        console::text::{Key, ScanCode},
        device_path::DevicePath,
        loaded_image::LoadedImage,
        media::block::BlockIO,
    },
};
use woven_installer::{plan, source_disk};

static IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_INSTALL_IMAGE"));

fn message(s: &str) {
    uefi::print!("{}", s);
}
fn line() -> Result<String> {
    let mut text = String::new();
    loop {
        let key = uefi::system::with_stdin(|input| -> Result<_> {
            let mut events = [input.wait_for_key_event()?];
            boot::wait_for_event(&mut events).map_err(|e| e.to_err_without_payload())?;
            input.read_key()
        })?;
        match key {
            Some(Key::Special(ScanCode::ESCAPE)) => return Err(Status::ABORTED.into()),
            Some(Key::Printable(c)) => match char::from(c) {
                '\r' => {
                    message("\n");
                    return Ok(text);
                }
                '\u{8}' if !text.is_empty() => {
                    text.pop();
                    message("\u{8} \u{8}");
                }
                c if (c.is_ascii_graphic() || c == ' ') && text.len() < 80 => {
                    text.push(c);
                    let mut s = String::new();
                    write!(&mut s, "{c}").unwrap();
                    message(&s);
                }
                _ => {}
            },
            _ => {}
        }
    }
}
struct Disk {
    handle: Handle,
    path: Vec<u8>,
    sectors: u64,
    media_id: u32,
    align: u32,
}

fn run() -> Result {
    message(
        "\nWovenHat experimental UEFI installer\nInstalls the development shell OS. All partitions on the selected disk are replaced.\nESC cancels before writing. Secure Boot must be disabled.\n",
    );
    let loaded = boot::open_protocol_exclusive::<LoadedImage>(boot::image_handle())?;
    let source = loaded.device().ok_or(Status::NOT_FOUND)?;
    drop(loaded);
    let source_path = boot::open_protocol_exclusive::<DevicePath>(source)?
        .as_bytes()
        .to_vec();
    let handles = boot::locate_handle_buffer(SearchType::ByProtocol(&BlockIO::GUID))?;
    let mut disks = Vec::new();
    for handle in handles.iter().copied() {
        // Copy identity before opening block I/O, which may disconnect child drivers.
        let Ok(path) = boot::open_protocol_exclusive::<DevicePath>(handle) else {
            continue;
        };
        let bytes = path.as_bytes().to_vec();
        let label = format!("{path}");
        drop(path);
        if source_disk(&bytes, &source_path) != Some(false) {
            continue;
        }
        let Ok(io) = boot::open_protocol_exclusive::<BlockIO>(handle) else {
            continue;
        };
        let m = io.media();
        if m.is_logical_partition()
            || !m.is_media_present()
            || m.is_read_only()
            || m.block_size() != 512
            || m.io_align() > 4096
            || (m.io_align() > 1 && !m.io_align().is_power_of_two())
            || disks.len() >= 16
        {
            continue;
        }
        let Some(sectors) = m.last_block().checked_add(1) else {
            continue;
        };
        if sectors < (IMAGE.len() / 512) as u64 {
            continue;
        }
        message(&format!(
            "Disk {}: {} MiB, {}\n",
            disks.len() + 1,
            sectors / 2048,
            label
        ));
        disks.push(Disk {
            handle,
            path: bytes,
            sectors,
            media_id: m.media_id(),
            align: m.io_align(),
        });
    }
    if disks.is_empty() {
        message("No eligible target disks. Nothing written.\n");
        return Err(Status::NOT_FOUND.into());
    }
    message("[INSTALL] SELECT DISK\nDisk number (or ESC): ");
    let choice = line()?
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .filter(|n| *n < disks.len())
        .ok_or(Status::INVALID_PARAMETER)?;
    let disk = &disks[choice];
    let install = plan(IMAGE, disk.sectors).map_err(|_| Status::COMPROMISED_DATA)?;
    message(&format!(
        "ERASE ALL PARTITIONS on Disk {} ({} MiB).\nType ERASE {} to install: ",
        choice + 1,
        disk.sectors / 2048,
        choice + 1
    ));
    if line()? != format!("ERASE {}", choice + 1) {
        message("Cancelled. Nothing written.\n");
        return Err(Status::ABORTED.into());
    }
    let path = boot::open_protocol_exclusive::<DevicePath>(disk.handle)?;
    if path.as_bytes() != disk.path || source_disk(path.as_bytes(), &source_path) != Some(false) {
        return Err(Status::MEDIA_CHANGED.into());
    }
    drop(path);
    let mut io = boot::open_protocol_exclusive::<BlockIO>(disk.handle)?;
    let m = io.media();
    if m.media_id() != disk.media_id
        || m.last_block() != disk.sectors - 1
        || m.block_size() != 512
        || m.io_align() != disk.align
        || m.is_read_only()
        || m.is_logical_partition()
        || !m.is_media_present()
    {
        return Err(Status::MEDIA_CHANGED.into());
    }
    message("[INSTALL] WRITING\n");
    // Aligned stack buffer supports firmware DMA alignment up to 4096.
    #[repr(align(4096))]
    struct Buffer([u8; 64 * 1024]);
    let mut buffer = Buffer([0; 64 * 1024]);
    for (base, data) in [
        (0, install.prefix.as_slice()),
        (install.table_lba, install.backup_table.as_slice()),
        (install.last_lba, install.backup_header.as_slice()),
    ] {
        for (i, chunk) in data.chunks(buffer.0.len()).enumerate() {
            buffer.0[..chunk.len()].copy_from_slice(chunk);
            io.write_blocks(
                disk.media_id,
                base + (i * buffer.0.len() / 512) as u64,
                &buffer.0[..chunk.len()],
            )?;
        }
    }
    io.flush_blocks()?;
    message("[INSTALL] VERIFYING\n");
    for (base, data) in [
        (0, install.prefix.as_slice()),
        (install.table_lba, install.backup_table.as_slice()),
        (install.last_lba, install.backup_header.as_slice()),
    ] {
        for (i, chunk) in data.chunks(buffer.0.len()).enumerate() {
            io.read_blocks(
                disk.media_id,
                base + (i * buffer.0.len() / 512) as u64,
                &mut buffer.0[..chunk.len()],
            )?;
            if &buffer.0[..chunk.len()] != chunk {
                return Err(Status::CRC_ERROR.into());
            }
        }
    }
    message(
        "[INSTALL] VERIFIED\nInstallation complete. Power off, remove installer USB, then select the target disk's UEFI boot entry.\nUnused disk space is unallocated. Old data outside written regions is not securely erased.\nPress Enter to return to firmware.\n",
    );
    let _ = line();
    Ok(())
}
#[entry]
fn main() -> Status {
    if uefi::helpers::init().is_err() {
        return Status::ABORTED;
    }
    match run() {
        Ok(()) => Status::SUCCESS,
        Err(error) => {
            message(&format!(
                "[INSTALL] STOPPED: {:?}\nIf writing started, the target may be incomplete. Press Enter.\n",
                error.status()
            ));
            let _ = line();
            error.status()
        }
    }
}
