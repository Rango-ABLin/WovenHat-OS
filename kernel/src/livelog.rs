//! Live-test log on the WovenHat USB stick (Stage 13.19).
//!
//! A WovenHat stick written by `scripts/write-live-usb.ps1` carries, after
//! its boot partition, a FAT32 partition labelled `WOVENLOG`. When a machine
//! boots from that stick, this module finds the partition on the USB disk
//! and keeps the boot's log there, so the log comes back with the stick:
//!
//! - `LOGS/BOOTnnnn.TXT`, a new file per boot, preallocated to [`CAPACITY`]
//!   bytes so later updates only rewrite data sectors in place: no FAT or
//!   directory sector changes after creation, and the file stays readable
//!   if the machine stops at any point.
//! - The first [`SUMMARY`] bytes are a summary rewritten on every flush:
//!   the boot number, how far the boot got, and every log line that reports
//!   a failure. The kernel log follows (see `serial::read_log`), padded with
//!   spaces to the file's fixed size.
//! - A worker flushes every [`FLUSH_TICKS`]; a panic flushes once more if
//!   the locks it needs are free.
//!
//! Only a FAT32 volume labelled `WOVENLOG` on the USB disk is ever written.

use alloc::string::String;
use alloc::vec::Vec;

use crate::block::{BlockDevice, SECTOR_SIZE};
use crate::irq_lock::IrqMutex;
use crate::{fat32, gpt, partition, serial, xhci};

const LABEL: &[u8; 11] = b"WOVENLOG   ";
const DIRECTORY: &str = "LOGS";
/// Bytes preallocated per boot log.
pub const CAPACITY: usize = 512 * 1024;
/// The summary at the start of the file.
pub const SUMMARY: usize = 4096;
/// Boot logs kept before numbering stops (`BOOT9999.TXT`).
const MAX_BOOTS: u32 = 9999;
/// Failure lines kept for the summary.
const MAX_ISSUES: usize = 40;
const ISSUE_CHARS: usize = 96;
/// Ticks between flushes (100 Hz).
const FLUSH_TICKS: u64 = 300;
/// Words that mark a log line as an issue.
const ISSUE_WORDS: [&str; 8] = ["FAIL", "Fail", "PANIC", "panic", "rror", "InitFailed", "Timeout", "denied"];

struct Log {
    partition: partition::Partition,
    /// Partition-relative LBA of every sector of the file, in order.
    sectors: Vec<u64>,
    boot: u32,
    /// Kernel log position shown at file offset [`SUMMARY`].
    base: usize,
    /// Kernel log position written so far.
    written: usize,
    /// The unfinished last line, scanned for issues once complete.
    partial: String,
    issues: Vec<String>,
    flushes: u64,
}

/// Held only to take the log out or put it back, never across I/O: a
/// flush owns the log while it writes, with no lock held.
static LOG: IrqMutex<Option<Log>> = IrqMutex::with_rank(None, 5);

/// Why the live log could not start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// No USB disk was attached at boot.
    NoUsbDisk,
    /// The USB disk has no FAT32 partition labelled WOVENLOG.
    NoLogPartition,
    /// The partition rejected the directory or file.
    Filesystem(fat32::Error),
    /// The new file's clusters could not be mapped.
    Layout,
    AlreadyStarted,
}

/// Find the WOVENLOG partition, create this boot's log file, write the
/// first flush, and start the worker. Returns the boot number.
pub fn start() -> Result<u32, Error> {
    if LOG.lock().is_some() {
        return Err(Error::AlreadyStarted);
    }
    let mut log = xhci::with_usb_disk(open).ok_or(Error::NoUsbDisk)??;
    let boot = log.boot;
    serial::write_line(format_args!(
        "[LIVELOG] writing LOGS/BOOT{:04}.TXT on the WovenHat USB stick ({} KiB)",
        boot,
        CAPACITY / 1024
    ));
    inventory();
    let _ = xhci::with_usb_disk(|disk| flush(&mut log, disk));
    *LOG.lock() = Some(log);
    if crate::task::spawn_io_service("livelog", worker).is_err() {
        serial::write_line(format_args!("[LIVELOG] worker did not start; the log is flushed only now"));
    }
    Ok(boot)
}

/// Flush now: everything logged so far reaches the stick. `false` when
/// there is no log, another flush owns it, or a write failed.
pub fn flush_now() -> bool {
    let Some(mut log) = LOG.lock().take() else { return false };
    let ok = xhci::with_usb_disk(|disk| flush(&mut log, disk)).unwrap_or(false);
    *LOG.lock() = Some(log);
    ok
}

/// A last flush from the panic handler, if neither the log nor the USB
/// controller is held by the code that panicked.
pub fn panic_flush() {
    if !xhci::controller_free() {
        return;
    }
    let Some(mut log) = LOG.try_lock().and_then(|mut slot| slot.take()) else { return };
    let _ = xhci::with_usb_disk(|disk| flush(&mut log, disk));
    if let Some(mut slot) = LOG.try_lock() {
        *slot = Some(log);
    }
}

fn worker() -> ! {
    loop {
        crate::task::sleep_current(FLUSH_TICKS);
        let _ = flush_now();
    }
}

fn open(disk: &mut xhci::UsbDisk) -> Result<Log, Error> {
    let partitions = gpt::fat_partitions(disk).map_err(|_| Error::NoLogPartition)?;
    let (partition, volume) = partitions
        .into_iter()
        .find_map(|partition| {
            let mut view = partition::PartitionDevice::new(&mut *disk, partition).ok()?;
            let volume = fat32::mount(&mut view).ok()?;
            let mut sector = [0u8; SECTOR_SIZE];
            view.read_sector(0, &mut sector).ok()?;
            // The label is in the boot sector, or (as Windows' format
            // writes it) only in the root directory's volume-label entry.
            let mut labelled = &sector[71..82] == LABEL;
            if !labelled {
                labelled = root_label(&mut view, volume) == Some(*LABEL);
            }
            labelled.then_some((partition, volume))
        })
        .ok_or(Error::NoLogPartition)?;
    let mut view = partition::PartitionDevice::new(&mut *disk, partition).map_err(|_| Error::NoLogPartition)?;
    match fat32::mkdir_path(&mut view, volume, DIRECTORY) {
        Ok(()) | Err(fat32::Error::AlreadyExists) => {}
        Err(error) => return Err(Error::Filesystem(error)),
    }
    let mut boot = 1;
    let path = loop {
        let path = alloc::format!("{}/BOOT{:04}.TXT", DIRECTORY, boot);
        match fat32::resolve_path(&mut view, volume, &path) {
            Err(fat32::Error::NotFound) => break path,
            Ok(_) if boot < MAX_BOOTS => boot += 1,
            Ok(_) => return Err(Error::Filesystem(fat32::Error::DirectoryFull)),
            Err(error) => return Err(Error::Filesystem(error)),
        }
    };
    let mut blank = alloc::vec![b' '; CAPACITY];
    blank[CAPACITY - 1] = b'\n';
    fat32::create_path_file(&mut view, volume, &path, &blank).map_err(Error::Filesystem)?;
    let entry = fat32::resolve_path(&mut view, volume, &path).map_err(Error::Filesystem)?;
    // Map the file: every sector of its cluster chain, in order.
    let mut sectors = Vec::with_capacity(CAPACITY / SECTOR_SIZE);
    let mut cluster = entry.first_cluster;
    while sectors.len() < CAPACITY / SECTOR_SIZE {
        let first = volume.cluster_lba(cluster).map_err(|_| Error::Layout)?;
        for sector in 0..u64::from(volume.sectors_per_cluster) {
            if sectors.len() < CAPACITY / SECTOR_SIZE {
                sectors.push(first + sector);
            }
        }
        if sectors.len() == CAPACITY / SECTOR_SIZE {
            break;
        }
        cluster = match fat32::next_cluster(&mut view, volume, cluster) {
            Ok(fat32::ClusterLink::Next(next)) => next,
            _ => return Err(Error::Layout),
        };
    }
    Ok(Log {
        partition,
        sectors,
        boot,
        base: 0,
        written: 0,
        partial: String::new(),
        issues: Vec::new(),
        flushes: 0,
    })
}

/// The volume-label entry in the first cluster of the root directory.
fn root_label(device: &mut impl BlockDevice, volume: fat32::Volume) -> Option<[u8; 11]> {
    let first = volume.cluster_lba(volume.root_cluster).ok()?;
    let mut sector = [0u8; SECTOR_SIZE];
    for index in 0..u64::from(volume.sectors_per_cluster) {
        device.read_sector(first + index, &mut sector).ok()?;
        for entry in sector.as_chunks::<32>().0 {
            match entry[0] {
                0 => return None,
                0xe5 => continue,
                // A volume label, not a long-name record (0x0f).
                _ if entry[11] & 0x3f == 0x08 => return entry[..11].try_into().ok(),
                _ => {}
            }
        }
    }
    None
}

/// Write the summary and every log byte not yet on the stick. Each pass
/// rewrites from the start of the sector holding the last write, so a byte
/// still being logged during one pass is corrected by the next.
fn flush(log: &mut Log, disk: &mut xhci::UsbDisk) -> bool {
    let head = serial::log_position();
    let capacity = CAPACITY - SUMMARY - 1;
    let end = head.min(log.base + capacity);
    // Scan the newly completed lines for issues.
    let mut fresh = alloc::vec![0u8; end.saturating_sub(log.written)];
    let (from, count) = serial::read_log(log.written, &mut fresh);
    if from > log.written {
        log.partial.clear();
    }
    for &byte in &fresh[..count] {
        if byte == b'\n' {
            let line = core::mem::take(&mut log.partial);
            if log.issues.len() < MAX_ISSUES && ISSUE_WORDS.iter().any(|word| line.contains(word)) {
                log.issues.push(line.chars().take(ISSUE_CHARS).collect());
            }
        } else if log.partial.len() < 512 {
            log.partial.push(if byte.is_ascii() && (byte >= 0x20 || byte == b'\t') { byte as char } else { '?' });
        }
    }
    log.flushes += 1;
    let Ok(mut view) = partition::PartitionDevice::new(disk, log.partition) else {
        return false;
    };
    let mut ok = true;
    // The summary.
    let summary = summary(log, head);
    for (index, chunk) in summary.chunks(SECTOR_SIZE).enumerate() {
        ok &= view.write_sector(log.sectors[index], chunk).is_ok();
    }
    // The log text, from the sector holding `written` to `end`.
    let first = (SUMMARY + log.written.saturating_sub(log.base)) / SECTOR_SIZE;
    let last = (SUMMARY + end.saturating_sub(log.base)).div_ceil(SECTOR_SIZE);
    let mut sector = [b' '; SECTOR_SIZE];
    for index in first..last.min(log.sectors.len()) {
        sector.fill(b' ');
        let position = log.base + index * SECTOR_SIZE - SUMMARY;
        let wanted = end.saturating_sub(position).min(SECTOR_SIZE);
        let (from, count) = serial::read_log(position, &mut sector[..wanted]);
        if from > position {
            // Overwritten in the ring before it reached the stick.
            sector[..wanted].fill(b'~');
        } else {
            for byte in &mut sector[..count] {
                if !(byte.is_ascii_graphic() || *byte == b' ' || *byte == b'\n' || *byte == b'\t') {
                    *byte = b'?';
                }
            }
        }
        if index == log.sectors.len() - 1 {
            sector[SECTOR_SIZE - 1] = b'\n';
        }
        ok &= view.write_sector(log.sectors[index], &sector).is_ok();
    }
    ok &= view.flush().is_ok();
    log.written = end;
    ok
}

fn summary(log: &Log, head: usize) -> Vec<u8> {
    use core::fmt::Write;
    let mut text = String::new();
    let _ = writeln!(text, "WovenHat OS live-test log, boot {:04}", log.boot);
    if let Some(time) = crate::rtc::read() {
        let _ = writeln!(
            text,
            "Machine clock: {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            time.year, time.month, time.day, time.hour, time.minute, time.second
        );
    }
    let _ = writeln!(text, "Kernel version: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(text, "Flushes: {}; log bytes: {}{}", log.flushes, head,
        if head > log.base + CAPACITY - SUMMARY - 1 { " (truncated: the file is full)" } else { "" });
    let _ = writeln!(text, "Issues found in the log: {}", log.issues.len());
    for issue in &log.issues {
        let _ = writeln!(text, "  {}", issue);
    }
    let mut bytes = text.into_bytes();
    let footer = b"\n---- kernel log follows ----\n";
    bytes.truncate(SUMMARY - footer.len());
    bytes.resize(SUMMARY - footer.len(), b' ');
    bytes.extend_from_slice(footer);
    bytes
}

/// What the machine is: written to the log once when it starts.
fn inventory() {
    let vendor = crate::hal::cpu::detect_vendor();
    serial::write_line(format_args!("[INVENTORY] cpu {:?} '{}', {} logical", vendor, cpu_brand().trim(),
        crate::hal::cpu::count_logical_cpus()));
    let memory = crate::memory::stats();
    serial::write_line(format_args!("[INVENTORY] memory {} MiB usable in {} regions",
        memory.total_frames * 4096 / (1024 * 1024), memory.usable_regions));
    match crate::terminal::framebuffer_info() {
        Some(info) => serial::write_line(format_args!("[INVENTORY] display {}x{} stride {} {:?}",
            info.width, info.height, info.stride, info.pixel_format)),
        None => serial::write_line(format_args!("[INVENTORY] display: no framebuffer")),
    }
    let mut index = 0;
    while let Some(device) = crate::hal::pci::device(index) {
        serial::write_line(format_args!(
            "[INVENTORY] pci {:04x}:{:02x}:{:02x}.{} {:04x}:{:04x} class {:02x}{:02x}{:02x} rev {:02x}",
            device.segment, device.bus, device.device, device.function, device.vendor_id, device.device_id,
            device.class, device.subclass, device.prog_if, device.revision
        ));
        index += 1;
    }
    if let Some(info) = xhci::usb_disk_info() {
        serial::write_line(format_args!("[INVENTORY] usb disk port {} '{}' '{}' {} blocks",
            info.port, core::str::from_utf8(&info.vendor).unwrap_or("?").trim_end(),
            core::str::from_utf8(&info.product).unwrap_or("?").trim_end(), info.blocks));
    }
}

/// The processor's brand string (CPUID 0x80000002..4).
fn cpu_brand() -> String {
    use core::arch::x86_64::__cpuid;
    // Leaf 0x80000000 reports whether the brand leaves exist.
    if __cpuid(0x8000_0000).eax < 0x8000_0004 {
        return String::from("unknown");
    }
    let mut bytes = Vec::with_capacity(48);
    for leaf in 0x8000_0002..=0x8000_0004u32 {
        let result = __cpuid(leaf);
        for register in [result.eax, result.ebx, result.ecx, result.edx] {
            bytes.extend_from_slice(&register.to_le_bytes());
        }
    }
    bytes.retain(|&byte| byte != 0);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Stage 13.19 acceptance: the boot's log on the WovenHat USB stick.
///
/// The stick is a GPT disk whose second partition is FAT32 labelled
/// WOVENLOG. The log must start on it as boot 1, refuse a second start,
/// and carry a tester's note, a reported failure (which must also reach the
/// summary) and a 200-line burst once flushed. The harness reads the file
/// back from the image and requires everything before the log partition to
/// be unchanged.
#[cfg(feature = "stage13-19-test")]
pub fn self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        serial::write_line(format_args!("[S13.19] {} {}", name, if ok { "PASS" } else { "FAIL" }));
        ok
    }
    let report = crate::drivers::bring_up();
    if !step("xhci0 bound and ready", matches!(report.drivers[2].outcome, crate::drivers::Outcome::Ready(_))) {
        return false;
    }
    if !step("a USB disk is attached", xhci::usb_disk_info().is_some()) {
        return false;
    }
    let started = start();
    if !step("the log starts on the WOVENLOG partition as boot 1", started == Ok(1)) {
        serial::write_line(format_args!("[S13.19] start: {:?}", started));
        return false;
    }
    if !step("a second start is refused", start() == Err(Error::AlreadyStarted)) {
        return false;
    }
    serial::write_line(format_args!("[NOTE] S13.19 tester note: the screen flickers on resume"));
    serial::write_line(format_args!("[S13.19] simulated driver FAILURE for the summary"));
    for line in 0..200 {
        serial::write_line(format_args!("[S13.19] burst line {:03} of the multi-sector log", line));
    }
    step("flushed to the stick", flush_now())
}
