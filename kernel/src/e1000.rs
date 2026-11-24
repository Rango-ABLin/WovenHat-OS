//! Intel e1000/e1000e PCI network transport for WovenHat OS.
//!
//! This is the first native (non-virtio) NIC driver. It targets the 82540EM as
//! exposed by QEMU's `-device e1000`; the 82542/82543GC/82545EM and the 82574L
//! (`-device e1000e`) reuse the same legacy MMIO register window (BAR0) and the
//! same legacy 16-byte descriptor rings, so they run through this dataplane
//! unchanged.
//!
//! As with [`crate::virtio_net`]: one MMIO register window, one bounded DMA
//! reservation holding the descriptor rings and their packet buffers, and a
//! single outstanding TX descriptor so smoltcp's serialized `Device::transmit`
//! contract holds.
//!
//! Interrupts are intentionally left masked (IMS cleared after reset): the
//! driver reaps completion through the descriptor `DD` bits inside the polling
//! loop instead, exactly like the legacy virtio-net dataplane. MSI/MSI-X and
//! interrupt moderation are follow-on work once the dataplane is stable.

use crate::irq_lock::IrqMutex as Mutex;
use core::{
    mem::size_of,
    ptr,
    sync::atomic::{fence, AtomicU64, Ordering},
};

use crate::{hal::pci, memory, paging};

/// Intel vendor ID, common to all e1000-family functions.
pub const INTEL_VENDOR: u16 = 0x8086;
/// QEMU's default `-device e1000` is the 82540EM. The 82542/82543GC share the
/// same programming model and are admitted here as well.
pub const E1000_82540EM: u16 = 0x100E;
const E1000_82542: u16 = 0x1000;
const E1000_82543GC: u16 = 0x1004;
const E1000_82545EM: u16 = 0x100F;
/// QEMU's `-device e1000e` is the 82574L. It keeps the legacy register window
/// and descriptor rings used here, so it is admitted by the same dataplane.
const E1000E_82574L: u16 = 0x10D3;

/// Largest Ethernet frame the transport will accept or submit, matching the
/// virtio-net bound so smoltcp sees a consistent MTU across transports.
pub const MAX_FRAME: usize = 1536;

// MMIO register offsets (dwords) within BAR0.
const REG_CTRL: u64 = 0x0000;
const REG_IMC: u64 = 0x00D8;
const REG_RCTL: u64 = 0x0100;
const REG_TCTL: u64 = 0x0400;
const REG_TIPG: u64 = 0x0410;
const REG_RDBAL: u64 = 0x2800;
const REG_RDBAH: u64 = 0x2804;
const REG_RDLEN: u64 = 0x2808;
const REG_RDH: u64 = 0x2810;
const REG_RDT: u64 = 0x2818;
const REG_TDBAL: u64 = 0x3800;
const REG_TDBAH: u64 = 0x3804;
const REG_TDLEN: u64 = 0x3808;
const REG_TDH: u64 = 0x3810;
const REG_TDT: u64 = 0x3818;
const REG_RAL: u64 = 0x5400;
const REG_RAH: u64 = 0x5404;

/// The register window this driver touches spans 0x0000..0x5404, i.e. six BAR
/// pages. They are mapped once during init and accessed directly afterward so
/// the polling dataplane never takes the paging lock underneath the transport
/// lock (which would invert their lock ranks).
const MMIO_PAGES: usize = 6;

// Device Control bits.
const CTRL_RST: u32 = 0x0400_0000;
const CTRL_FD: u32 = 0x0010_0000;
const CTRL_SLU: u32 = 0x0000_0040;
const CTRL_ASDE: u32 = 0x0000_0020;

// Receive Control bits. BSIZE=0 selects 2048-byte buffers; SECRC strips the
// trailing FCS so smoltcp receives a plain Ethernet frame.
const RCTL_EN: u32 = 0x0000_0002;
const RCTL_SBP: u32 = 0x0000_0004;
const RCTL_BAM: u32 = 0x0000_8000;
const RCTL_SECRC: u32 = 0x0400_0000;

// Transmit Control bits: enable, pad short packets, collision threshold 15,
// collision distance 64 (full-duplex friendly defaults).
const TCTL_EN: u32 = 0x0000_0002;
const TCTL_PSP: u32 = 0x0000_0008;
const TCTL_COLLISION: u32 = 0x0000_00F0 | 0x0004_0000;

// Legacy descriptor status/command bits.
const RX_STATUS_DD: u8 = 0x01;
const RX_STATUS_EOP: u8 = 0x02;
const TX_CMD_EOP: u8 = 0x01;
const TX_CMD_IFCS: u8 = 0x02;
const TX_CMD_RS: u8 = 0x08;
const TX_STATUS_DD: u8 = 0x01;

const PAGE_SIZE: usize = 4096;
const DESC_BYTES: usize = 16;
const PACKET_BYTES: usize = 2048;
/// Ring sizes must each produce a descriptor table at least 128 bytes long so
/// RDLEN/TDLEN satisfy the hardware's 128-byte alignment.
const RX_DESC_COUNT: usize = 8;
const TX_DESC_COUNT: usize = 8;

// DMA reservation layout. Every offset is 16-byte aligned so descriptor-ring
// physical bases meet the hardware's 16-byte alignment requirement.
const RX_DESC_OFFSET: usize = 0;
const RX_DESC_BYTES: usize = RX_DESC_COUNT * DESC_BYTES; // 128
const TX_DESC_OFFSET: usize = RX_DESC_OFFSET + RX_DESC_BYTES; // 128
const TX_DESC_BYTES: usize = TX_DESC_COUNT * DESC_BYTES; // 128
const RX_BUF_OFFSET: usize = TX_DESC_OFFSET + TX_DESC_BYTES; // 256
const RX_BUF_BYTES: usize = RX_DESC_COUNT * PACKET_BYTES; // 16384
const TX_BUF_OFFSET: usize = RX_BUF_OFFSET + RX_BUF_BYTES; // 16640
const TX_BUF_BYTES: usize = TX_DESC_COUNT * PACKET_BYTES; // 16384
const DMA_BYTES: usize = TX_BUF_OFFSET + TX_BUF_BYTES; // 33024
const DMA_PAGES: usize = DMA_BYTES.div_ceil(PAGE_SIZE); // 9

/// Reset completion poll budget; bounded so a wedged device fails instead of
/// hanging the boot.
const RESET_POLL_LIMIT: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciLocation {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeStatus {
    Missing,
    Found(PciLocation),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitError {
    Missing,
    NoMemoryBar,
    MmioUnavailable,
    DmaNotContiguous,
    ResetTimeout,
}

/// Legacy e1000 receive descriptor (16 bytes).
#[repr(C)]
#[derive(Clone, Copy)]
struct RxDesc {
    addr: u64,
    length: u16,
    checksum: u16,
    status: u8,
    errors: u8,
    special: u16,
}

/// Legacy e1000 transmit descriptor (16 bytes).
#[repr(C)]
#[derive(Clone, Copy)]
struct TxDesc {
    addr: u64,
    length: u16,
    cso: u8,
    cmd: u8,
    status: u8,
    css: u8,
    special: u16,
}

// The bootloader's static pages are not a DMA contiguity contract, so ring and
// packet storage live in frames reserved as one physical run and are reached
// through the direct physical-memory mapping, mirroring virtio_net.
static DMA_PHYSICAL: AtomicU64 = AtomicU64::new(0);
static DMA_VIRTUAL: AtomicU64 = AtomicU64::new(0);

struct DmaReservation {
    frame: x86_64::structures::paging::PhysFrame<x86_64::structures::paging::Size4KiB>,
    pages: usize,
    committed: bool,
}

impl DmaReservation {
    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for DmaReservation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        DMA_PHYSICAL.store(0, Ordering::Release);
        DMA_VIRTUAL.store(0, Ordering::Release);
        let start = self.frame.start_address().as_u64();
        for index in 0..self.pages {
            if let Ok(frame) = x86_64::structures::paging::PhysFrame::from_start_address(
                x86_64::PhysAddr::new(start + (index as u64 * PAGE_SIZE as u64)),
            ) {
                let _ = memory::deallocate_frame(frame);
            }
        }
    }
}

fn dma_ptr(offset: usize) -> *mut u8 {
    (DMA_VIRTUAL.load(Ordering::Acquire) + offset as u64) as *mut u8
}

fn dma_phys(offset: usize) -> u64 {
    DMA_PHYSICAL.load(Ordering::Acquire) + offset as u64
}

fn reserve_dma() -> Result<DmaReservation, InitError> {
    let frame = memory::allocate_contiguous_frames(DMA_PAGES).ok_or(InitError::DmaNotContiguous)?;
    let physical = frame.start_address().as_u64();
    let offset = paging::physical_memory_offset().ok_or(InitError::DmaNotContiguous)?;
    let virtual_address = offset
        .checked_add(physical)
        .ok_or(InitError::DmaNotContiguous)?;
    DMA_PHYSICAL.store(physical, Ordering::Release);
    DMA_VIRTUAL.store(virtual_address, Ordering::Release);
    // SAFETY: `virtual_address` is the direct-map address of the DMA_PAGES
    // physically-contiguous frames just reserved for this driver, so the whole
    // `DMA_BYTES` span is mapped, writable and not aliased by any other owner.
    unsafe {
        ptr::write_bytes(virtual_address as *mut u8, 0, DMA_BYTES);
    }
    Ok(DmaReservation {
        frame,
        pages: DMA_PAGES,
        committed: false,
    })
}

fn rx_desc(index: usize) -> *mut RxDesc {
    // SAFETY: `index < RX_DESC_COUNT` is guaranteed by callers, so the offset
    // into the RX descriptor sub-region of the DMA reservation stays in bounds.
    unsafe { dma_ptr(RX_DESC_OFFSET).add(index * DESC_BYTES) as *mut RxDesc }
}

fn tx_desc(index: usize) -> *mut TxDesc {
    // SAFETY: `index < TX_DESC_COUNT` is guaranteed by callers, so the offset
    // into the TX descriptor sub-region of the DMA reservation stays in bounds.
    unsafe { dma_ptr(TX_DESC_OFFSET).add(index * DESC_BYTES) as *mut TxDesc }
}

#[derive(Clone, Copy)]
struct Transport {
    initialized: bool,
    mmio: [u64; MMIO_PAGES],
    rx_tail: u32,
    tx_tail: usize,
    tx_outstanding: bool,
    mac: [u8; 6],
    rx_frames: u64,
    tx_frames: u64,
    rx_dropped: u64,
    tx_busy: u64,
}

impl Transport {
    const fn empty() -> Self {
        Self {
            initialized: false,
            mmio: [0; MMIO_PAGES],
            rx_tail: 0,
            tx_tail: 0,
            tx_outstanding: false,
            mac: [0; 6],
            rx_frames: 0,
            tx_frames: 0,
            rx_dropped: 0,
            tx_busy: 0,
        }
    }
}

static TRANSPORT: Mutex<Transport> = Mutex::with_rank(Transport::empty(), 30);

/// Locate the first e1000-family network function. Unsupported Intel NICs are
/// left for a later driver rather than matched by class alone.
pub fn probe() -> ProbeStatus {
    let mut index = 0usize;
    while let Some(dev) = pci::device(index) {
        if dev.vendor_id == INTEL_VENDOR
            && matches!(
                dev.device_id,
                E1000_82540EM | E1000_82542 | E1000_82543GC | E1000_82545EM | E1000E_82574L
            )
            && dev.class == 0x02
        {
            return ProbeStatus::Found(PciLocation {
                bus: dev.bus,
                device: dev.device,
                function: dev.function,
            });
        }
        index += 1;
    }
    ProbeStatus::Missing
}

pub fn init() -> Result<(), InitError> {
    let location = match probe() {
        ProbeStatus::Missing => return Err(InitError::Missing),
        ProbeStatus::Found(location) => location,
    };
    let dev = find_device(location).ok_or(InitError::Missing)?;

    // BAR0 is the MMIO register window for every device this driver admits.
    let bar = dev
        .bars
        .iter()
        .find(|bar| {
            bar.valid && matches!(bar.kind, pci::BarKind::Memory32 | pci::BarKind::Memory64)
        })
        .ok_or(InitError::NoMemoryBar)?;
    let bar0 = bar.address;
    if bar0 == 0 {
        return Err(InitError::NoMemoryBar);
    }

    let address = pci::Address {
        segment: dev.segment,
        bus: dev.bus,
        device: dev.device,
        function: dev.function,
    };
    if !pci::enable_memory_bus_master(address) {
        return Err(InitError::Missing);
    }

    let mmio = map_registers(bar0)?;

    // Reset, then mask every interrupt source: this transport is polling-only
    // and must not assert an unmasked line that no handler acknowledges.
    mmio_write32(&mmio, REG_CTRL, mmio_read32(&mmio, REG_CTRL) | CTRL_RST);
    let mut reset_done = false;
    for _ in 0..RESET_POLL_LIMIT {
        if mmio_read32(&mmio, REG_CTRL) & CTRL_RST == 0 {
            reset_done = true;
            break;
        }
        core::hint::spin_loop();
    }
    if !reset_done {
        return Err(InitError::ResetTimeout);
    }
    mmio_write32(&mmio, REG_IMC, 0xFFFF_FFFF);

    // Force link up and full duplex: the QEMU model reports link immediately
    // and there is no PHY negotiation for the admitted device family.
    let ctrl = mmio_read32(&mmio, REG_CTRL) | CTRL_FD | CTRL_SLU | CTRL_ASDE;
    mmio_write32(&mmio, REG_CTRL, ctrl);

    // Read the MAC out of the receive-address registers, then re-assert it with
    // the address-valid bit so the hardware filters on it.
    let ral = mmio_read32(&mmio, REG_RAL);
    let rah = mmio_read32(&mmio, REG_RAH);
    let mut mac = [0u8; 6];
    mac[0] = ral as u8;
    mac[1] = (ral >> 8) as u8;
    mac[2] = (ral >> 16) as u8;
    mac[3] = (ral >> 24) as u8;
    mac[4] = rah as u8;
    mac[5] = (rah >> 8) as u8;
    mmio_write32(&mmio, REG_RAL, ral);
    mmio_write32(&mmio, REG_RAH, 0x8000_0000 | (rah & 0x0000_FFFF));

    let dma = reserve_dma()?;
    setup_rings(&mmio);

    *TRANSPORT.lock() = Transport {
        initialized: true,
        mmio,
        rx_tail: RX_DESC_COUNT as u32 - 1,
        tx_tail: 0,
        tx_outstanding: false,
        mac,
        rx_frames: 0,
        tx_frames: 0,
        rx_dropped: 0,
        tx_busy: 0,
    };
    dma.commit();
    Ok(())
}

fn setup_rings(mmio: &[u64; MMIO_PAGES]) {
    // RX: every descriptor owns one packet buffer; the tail is left one behind
    // the head so all buffers are initially owned by the device.
    for index in 0..RX_DESC_COUNT {
        // SAFETY: `index < RX_DESC_COUNT`, and the device has not been enabled
        // yet, so this write is the single owner of the descriptor.
        unsafe {
            (*rx_desc(index)) = RxDesc {
                addr: dma_phys(RX_BUF_OFFSET + index * PACKET_BYTES),
                length: 0,
                checksum: 0,
                status: 0,
                errors: 0,
                special: 0,
            };
        }
    }
    mmio_write32(mmio, REG_RDBAL, dma_phys(RX_DESC_OFFSET) as u32);
    mmio_write32(mmio, REG_RDBAH, (dma_phys(RX_DESC_OFFSET) >> 32) as u32);
    mmio_write32(mmio, REG_RDLEN, RX_DESC_BYTES as u32);
    mmio_write32(mmio, REG_RDH, 0);
    mmio_write32(mmio, REG_RDT, RX_DESC_COUNT as u32 - 1);
    mmio_write32(mmio, REG_RCTL, RCTL_EN | RCTL_SBP | RCTL_BAM | RCTL_SECRC);

    // TX: every descriptor owns its own packet buffer and the head/tail
    // counters begin equal (idle). Submission advances round-robin through the
    // ring, so the absolute TDT counter always maps to the next descriptor.
    for index in 0..TX_DESC_COUNT {
        // SAFETY: `index < TX_DESC_COUNT`, single owner before device enable.
        unsafe {
            (*tx_desc(index)) = TxDesc {
                addr: dma_phys(TX_BUF_OFFSET + index * PACKET_BYTES),
                length: 0,
                cso: 0,
                cmd: 0,
                status: 0,
                css: 0,
                special: 0,
            };
        }
    }
    mmio_write32(mmio, REG_TDBAL, dma_phys(TX_DESC_OFFSET) as u32);
    mmio_write32(mmio, REG_TDBAH, (dma_phys(TX_DESC_OFFSET) >> 32) as u32);
    mmio_write32(mmio, REG_TDLEN, TX_DESC_BYTES as u32);
    mmio_write32(mmio, REG_TDH, 0);
    mmio_write32(mmio, REG_TDT, 0);
    mmio_write32(mmio, REG_TCTL, TCTL_EN | TCTL_PSP | TCTL_COLLISION);
    // Inter-packet gap: IPGT=10, IPGR1=8, IPGR2=6 (82540EM defaults).
    mmio_write32(mmio, REG_TIPG, 10 | (8 << 10) | (6 << 20));
}

pub fn is_initialized() -> bool {
    TRANSPORT.lock().initialized
}

/// Dataplane counters, mirroring `virtio_net::Stats`' counter set so the
/// diagnostic shell can report either transport through one shape.
// The reporting caller is not wired yet; keep the shape available without a
// broad module-level suppression.
#[derive(Clone, Copy, Default)]
pub struct Counters {
    #[allow(dead_code)]
    pub initialized: bool,
    pub rx_frames: u64,
    pub tx_frames: u64,
    pub rx_dropped: u64,
    pub tx_busy: u64,
}

pub fn counters() -> Counters {
    let t = TRANSPORT.lock();
    Counters {
        initialized: t.initialized,
        rx_frames: t.rx_frames,
        tx_frames: t.tx_frames,
        rx_dropped: t.rx_dropped,
        tx_busy: t.tx_busy,
    }
}

pub fn mac_address() -> [u8; 6] {
    TRANSPORT.lock().mac
}

/// Report whether the single TX descriptor can accept a frame. Completion is
/// reaped first so smoltcp is never handed a token that would silently drop.
pub fn tx_available() -> bool {
    let mut t = TRANSPORT.lock();
    if !t.initialized {
        return false;
    }
    reap_tx(&mut t);
    !t.tx_outstanding
}

/// Poll device completion. Reading completion through the descriptor `DD` bits
/// is race-safe because the device is the only other writer and interrupts are
/// masked, so no ICR acknowledgement is required.
pub fn poll() {
    let mut t = TRANSPORT.lock();
    if t.initialized {
        reap_tx(&mut t);
    }
}

pub fn receive_into(out: &mut [u8]) -> Option<usize> {
    let mut t = TRANSPORT.lock();
    if !t.initialized {
        return None;
    }
    let next = (t.rx_tail as usize + 1) % RX_DESC_COUNT;
    // SAFETY: `next < RX_DESC_COUNT`; the DD bit is device-written, so it is
    // read volatile under the transport lock.
    let desc = unsafe { &*rx_desc(next) };
    let status = unsafe { ptr::read_volatile(&desc.status as *const u8) };
    if status & RX_STATUS_DD == 0 {
        return None;
    }
    if status & RX_STATUS_EOP == 0 {
        // A non-EOP fragment: this driver posts only whole-frame buffers, so a
        // non-EOP descriptor indicates a descriptor/buffer contract violation.
        // Return the buffer and drop the fragment rather than corrupt a frame.
        t.rx_dropped = t.rx_dropped.saturating_add(1);
        advance_rx(&mut t);
        return None;
    }
    fence(Ordering::Acquire);
    let frame_len = usize::from(desc.length).min(MAX_FRAME);
    if out.len() < frame_len {
        t.rx_dropped = t.rx_dropped.saturating_add(1);
        advance_rx(&mut t);
        return None;
    }
    let packet = dma_ptr(RX_BUF_OFFSET + next * PACKET_BYTES) as *const u8;
    // SAFETY: `next < RX_DESC_COUNT`, so `packet` is that descriptor's buffer
    // in the DMA region; `frame_len` was clamped to both the device-reported
    // length and `out.len()`, and the kernel buffer cannot alias the caller's.
    unsafe {
        ptr::copy_nonoverlapping(packet, out.as_mut_ptr(), frame_len);
    }
    t.rx_frames = t.rx_frames.saturating_add(1);
    advance_rx(&mut t);
    Some(frame_len)
}

fn advance_rx(t: &mut Transport) {
    // Clear DD (and the whole status byte) before handing the descriptor back,
    // then advance RDT so the buffer becomes device-owned again.
    let index = (t.rx_tail as usize + 1) % RX_DESC_COUNT;
    // SAFETY: `index < RX_DESC_COUNT`; the descriptor was just reaped.
    unsafe {
        ptr::write_volatile(&mut (*rx_desc(index)).status as *mut u8, 0);
    }
    fence(Ordering::Release);
    // RDT names the most recently returned descriptor; keeping it a ring index
    // (never an absolute frame counter) matches the hardware's modulo head/tail
    // comparison. The window is pre-mapped, so this takes no paging lock.
    t.rx_tail = index as u32;
    mmio_write32(&t.mmio, REG_RDT, t.rx_tail);
}

pub fn transmit(frame: &[u8]) -> bool {
    if frame.is_empty() || frame.len() > MAX_FRAME {
        return false;
    }
    let mut t = TRANSPORT.lock();
    if !t.initialized {
        return false;
    }
    reap_tx(&mut t);
    if t.tx_outstanding {
        t.tx_busy = t.tx_busy.saturating_add(1);
        return false;
    }

    let index = t.tx_tail % TX_DESC_COUNT;
    let packet = dma_ptr(TX_BUF_OFFSET + index * PACKET_BYTES);
    // SAFETY: `packet` is descriptor `index`'s buffer in the DMA region;
    // `frame.len() <= MAX_FRAME < PACKET_BYTES` was checked, and `frame` cannot
    // alias the kernel DMA region.
    unsafe {
        ptr::copy_nonoverlapping(frame.as_ptr(), packet, frame.len());
    }
    // SAFETY: `index < TX_DESC_COUNT`; the descriptor is free while
    // `tx_outstanding` is false, so no device or CPU reference aliases it.
    unsafe {
        let desc = &mut *tx_desc(index);
        desc.length = frame.len() as u16;
        desc.cmd = TX_CMD_EOP | TX_CMD_IFCS | TX_CMD_RS;
        ptr::write_volatile(&mut desc.status as *mut u8, 0);
    }
    fence(Ordering::Release);
    // TDT is the next slot software will fill, expressed as a ring index so it
    // wraps with the hardware's modulo head compare rather than growing into an
    // out-of-range value that would replay stale descriptors.
    t.tx_tail = (t.tx_tail + 1) % TX_DESC_COUNT;
    mmio_write32(&t.mmio, REG_TDT, t.tx_tail as u32);
    t.tx_outstanding = true;
    t.tx_frames = t.tx_frames.saturating_add(1);
    true
}

fn reap_tx(t: &mut Transport) {
    if !t.tx_outstanding {
        return;
    }
    let index = (t.tx_tail + TX_DESC_COUNT - 1) % TX_DESC_COUNT;
    // SAFETY: `index < TX_DESC_COUNT`; the descriptor's DD bit is
    // device-written, so it is read volatile.
    let status = unsafe { ptr::read_volatile(&(*tx_desc(index)).status as *const u8) };
    if status & TX_STATUS_DD != 0 {
        fence(Ordering::Acquire);
        t.tx_outstanding = false;
    }
}

fn find_device(location: PciLocation) -> Option<pci::Device> {
    let mut index = 0usize;
    while let Some(dev) = pci::device(index) {
        if dev.bus == location.bus
            && dev.device == location.device
            && dev.function == location.function
        {
            return Some(dev);
        }
        index += 1;
    }
    None
}

/// Read one register dword from a pre-mapped BAR window. The caller guarantees
/// `offset < MMIO_PAGES * PAGE_SIZE`; every register offset in this module is a
/// compile-time constant below 0x6000.
fn mmio_read32(mmio: &[u64; MMIO_PAGES], offset: u64) -> u32 {
    let page = (offset / PAGE_SIZE as u64) as usize;
    let page_offset = (offset % PAGE_SIZE as u64) as usize;
    // SAFETY: `mmio[page] + page_offset` is the uncached mapping of this BAR
    // page, established during init and stable for the transport's lifetime.
    unsafe { ptr::read_volatile((mmio[page] + page_offset as u64) as *const u32) }
}

fn mmio_write32(mmio: &[u64; MMIO_PAGES], offset: u64, value: u32) {
    let page = (offset / PAGE_SIZE as u64) as usize;
    let page_offset = (offset % PAGE_SIZE as u64) as usize;
    // SAFETY: same mapping derivation as `mmio_read32`.
    unsafe { ptr::write_volatile((mmio[page] + page_offset as u64) as *mut u32, value) };
}

/// Map every BAR page this driver touches. Mapping is performed before the
/// transport lock is taken, so the dataplane can access the window without
/// re-entering the paging lock.
fn map_registers(bar0: u64) -> Result<[u64; MMIO_PAGES], InitError> {
    let mut mmio = [0u64; MMIO_PAGES];
    for (index, slot) in mmio.iter_mut().enumerate() {
        let physical = bar0
            .checked_add((index * PAGE_SIZE) as u64)
            .ok_or(InitError::MmioUnavailable)?;
        *slot = paging::map_mmio(physical).map_err(|_| InitError::MmioUnavailable)?;
    }
    Ok(mmio)
}

pub fn self_test() -> bool {
    size_of::<RxDesc>() == DESC_BYTES
        && size_of::<TxDesc>() == DESC_BYTES
        && RX_DESC_BYTES.is_multiple_of(128)
        && TX_DESC_BYTES.is_multiple_of(128)
        && RX_DESC_OFFSET.is_multiple_of(16)
        && TX_DESC_OFFSET.is_multiple_of(16)
        && RX_BUF_OFFSET.is_multiple_of(16)
        && TX_BUF_OFFSET.is_multiple_of(16)
        && TX_BUF_OFFSET + TX_BUF_BYTES == DMA_BYTES
        && MAX_FRAME <= PACKET_BYTES
}
