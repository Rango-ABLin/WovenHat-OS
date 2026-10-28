//! VirtIO-net PCI transport for WovenHat OS.
//!
//! Stage 3 implements the transitional/legacy PCI I/O transport used by QEMU
//! with `virtio-net-pci,disable-modern=on`. It owns real split virtqueues,
//! DMA descriptor tables, RX buffers and a TX buffer. The implementation is
//! intentionally polling-based first; MSI-X/interrupt moderation can be added
//! after the dataplane is stable.

use crate::irq_lock::IrqMutex as Mutex;
use core::{
    mem::size_of,
    sync::atomic::{fence, AtomicU64, Ordering},
};

use crate::{hal::pci, memory, paging};

pub const VIRTIO_VENDOR: u16 = 0x1af4;
pub const VIRTIO_NET_LEGACY: u16 = 0x1000;
pub const VIRTIO_NET_MODERN: u16 = 0x1041;
pub const MAX_FRAME: usize = 1536;

const VIRTIO_PCI_HOST_FEATURES: u16 = 0x00;
const VIRTIO_PCI_GUEST_FEATURES: u16 = 0x04;
const VIRTIO_PCI_QUEUE_PFN: u16 = 0x08;
const VIRTIO_PCI_QUEUE_NUM: u16 = 0x0c;
const VIRTIO_PCI_QUEUE_SEL: u16 = 0x0e;
const VIRTIO_PCI_QUEUE_NOTIFY: u16 = 0x10;
const VIRTIO_PCI_STATUS: u16 = 0x12;
const VIRTIO_PCI_ISR: u16 = 0x13;
const VIRTIO_PCI_CONFIG: u16 = 0x14;

const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FAILED: u8 = 128;

const DESC_F_WRITE: u16 = 2;
const RX_QUEUE: u16 = 0;
const TX_QUEUE: u16 = 1;
const MAX_QUEUE_SIZE: usize = 256;
const ACTIVE_RX_DESCRIPTORS: usize = 8;
const QUEUE_MEMORY_BYTES: usize = 16 * 1024;
const PACKET_BYTES: usize = 2048;
const VIRTIO_NET_HDR_BYTES: usize = 10;
const PAGE_SIZE: usize = 4096;

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
    ModernOnly,
    NoIoBar,
    QueueUnavailable,
    QueueTooLarge,
    DmaNotContiguous,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct Stats {
    pub initialized: bool,
    pub rx_frames: u64,
    pub tx_frames: u64,
    pub rx_dropped: u64,
    pub tx_busy: u64,
    pub host_features: u32,
    pub io_base: u16,
    pub rx_queue_size: u16,
    pub tx_queue_size: u16,
    pub mac: [u8; 6],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

const RX_QUEUE_OFFSET: usize = 0;
const TX_QUEUE_OFFSET: usize = RX_QUEUE_OFFSET + QUEUE_MEMORY_BYTES;
const RX_PACKETS_OFFSET: usize = TX_QUEUE_OFFSET + QUEUE_MEMORY_BYTES;
const RX_PACKETS_BYTES: usize = ACTIVE_RX_DESCRIPTORS * PACKET_BYTES;
const TX_PACKET_OFFSET: usize = RX_PACKETS_OFFSET + RX_PACKETS_BYTES;
const DMA_BYTES: usize = TX_PACKET_OFFSET + PACKET_BYTES;
const DMA_PAGES: usize = DMA_BYTES.div_ceil(PAGE_SIZE);

// The bootloader's static BSS pages are not a DMA contiguity contract. Keep
// virtqueue and packet storage in frames explicitly reserved as one physical
// run, then access that run through the direct physical-memory mapping.
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
    let reservation = DmaReservation {
        frame,
        pages: DMA_PAGES,
        committed: false,
    };
    let physical = frame.start_address().as_u64();
    let offset = match paging::physical_memory_offset() {
        Some(offset) => offset,
        None => return Err(InitError::DmaNotContiguous),
    };
    let virtual_address = offset
        .checked_add(physical)
        .ok_or(InitError::DmaNotContiguous)?;
    DMA_PHYSICAL.store(physical, Ordering::Release);
    DMA_VIRTUAL.store(virtual_address, Ordering::Release);
    // SAFETY: `virtual_address` is the direct-map address of the DMA_PAGES
    // physically-contiguous frames just reserved for this driver, so the whole
    // `DMA_BYTES` span is mapped, writable and not aliased by any other owner.
    unsafe {
        core::ptr::write_bytes(virtual_address as *mut u8, 0, DMA_BYTES);
    }
    Ok(reservation)
}

#[derive(Clone, Copy)]
struct Transport {
    initialized: bool,
    #[allow(dead_code)]
    location: PciLocation,
    io_base: u16,
    host_features: u32,
    rx_queue_size: u16,
    tx_queue_size: u16,
    rx_last_used: u16,
    tx_last_used: u16,
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
            location: PciLocation {
                bus: 0,
                device: 0,
                function: 0,
            },
            io_base: 0,
            host_features: 0,
            rx_queue_size: 0,
            tx_queue_size: 0,
            rx_last_used: 0,
            tx_last_used: 0,
            tx_outstanding: false,
            mac: [0x02, 0x57, 0x48, 0, 0, 1],
            rx_frames: 0,
            tx_frames: 0,
            rx_dropped: 0,
            tx_busy: 0,
        }
    }
}

static TRANSPORT: Mutex<Transport> = Mutex::with_rank(Transport::empty(), 30);

pub fn probe() -> ProbeStatus {
    let mut index = 0usize;
    while let Some(dev) = pci::device(index) {
        if dev.vendor_id == VIRTIO_VENDOR
            && matches!(dev.device_id, VIRTIO_NET_LEGACY | VIRTIO_NET_MODERN)
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
    if dev.device_id != VIRTIO_NET_LEGACY {
        return Err(InitError::ModernOnly);
    }

    pci::enable_io_bus_master(location.bus, location.device, location.function);
    let io_base = pci::bar0_io_base(location.bus, location.device, location.function)
        .ok_or(InitError::NoIoBar)?;

    // SAFETY: `io_base` is this device's BAR0 I/O-port base from
    // `pci::bar0_io_base`, and `VIRTIO_PCI_STATUS` is the fixed legacy
    // virtio-pci status register within that BAR. Writing reset (0) then the
    // ACKNOWLEDGE/DRIVER handshake is the spec-defined bring-up sequence.
    unsafe {
        outb(io_base + VIRTIO_PCI_STATUS, 0);
        outb(io_base + VIRTIO_PCI_STATUS, STATUS_ACKNOWLEDGE);
        outb(
            io_base + VIRTIO_PCI_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER,
        );
    }

    // SAFETY: host-features register read on the validated I/O BAR.
    let host_features = unsafe { inl(io_base + VIRTIO_PCI_HOST_FEATURES) };
    // Start conservatively: no checksum/GSO/event-index/indirect features.
    // SAFETY: writing the negotiated (empty) guest feature set to the validated
    // I/O BAR.
    unsafe {
        outl(io_base + VIRTIO_PCI_GUEST_FEATURES, 0);
    }

    let dma = reserve_dma()?;
    let rx_queue_size = setup_queue(io_base, RX_QUEUE).inspect_err(|error| {
        crate::serial::write_line(format_args!(
            "[VIRTIO-NET] RX queue setup failed: {:?}",
            error
        ));
    })?;
    let tx_queue_size = setup_queue(io_base, TX_QUEUE).inspect_err(|error| {
        crate::serial::write_line(format_args!(
            "[VIRTIO-NET] TX queue setup failed: {:?}",
            error
        ));
    })?;
    post_initial_rx(io_base, rx_queue_size).inspect_err(|error| {
        crate::serial::write_line(format_args!("[VIRTIO-NET] RX posting failed: {:?}", error));
    })?;
    setup_tx_descriptor(tx_queue_size).inspect_err(|error| {
        crate::serial::write_line(format_args!(
            "[VIRTIO-NET] TX descriptor setup failed: {:?}",
            error
        ));
    })?;

    let mut mac = [0x02, 0x57, 0x48, 0, 0, 1];
    // VIRTIO_NET_F_MAC is feature bit 5. QEMU normally exposes it.
    if host_features & (1 << 5) != 0 {
        for (i, byte) in mac.iter_mut().enumerate() {
            // SAFETY: VIRTIO_NET_F_MAC was negotiated, so the 6-byte MAC lives
            // at CONFIG+i (i < 6) in the device-config space of the validated
            // I/O BAR.
            *byte = unsafe { inb(io_base + VIRTIO_PCI_CONFIG + i as u16) };
        }
    }

    // SAFETY: final DRIVER_OK status write on the validated I/O BAR completes
    // device bring-up.
    unsafe {
        outb(
            io_base + VIRTIO_PCI_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK,
        );
    }

    *TRANSPORT.lock() = Transport {
        initialized: true,
        location,
        io_base,
        host_features,
        rx_queue_size,
        tx_queue_size,
        rx_last_used: 0,
        tx_last_used: 0,
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

pub fn is_initialized() -> bool {
    TRANSPORT.lock().initialized
}

pub fn mac_address() -> [u8; 6] {
    TRANSPORT.lock().mac
}

/// Report whether the single polling-mode TX descriptor can accept a frame.
///
/// smoltcp treats `Device::transmit() -> Some(_)` as a promise that consuming
/// the returned token can submit the packet. Reap completion first so callers
/// do not receive a token while the descriptor is still owned by the device.
pub fn tx_available() -> bool {
    let mut t = TRANSPORT.lock();
    if !t.initialized {
        return false;
    }
    reap_tx(&mut t);
    !t.tx_outstanding
}

pub fn stats() -> Stats {
    let t = *TRANSPORT.lock();
    Stats {
        initialized: t.initialized,
        rx_frames: t.rx_frames,
        tx_frames: t.tx_frames,
        rx_dropped: t.rx_dropped,
        tx_busy: t.tx_busy,
        host_features: t.host_features,
        io_base: t.io_base,
        rx_queue_size: t.rx_queue_size,
        tx_queue_size: t.tx_queue_size,
        mac: t.mac,
    }
}

pub fn poll() {
    let mut t = TRANSPORT.lock();
    if !t.initialized {
        return;
    }
    reap_tx(&mut t);
    // Reading ISR acknowledges any pending legacy interrupt. We still use the
    // used-ring indices as the source of truth, so polling remains race-safe.
    // SAFETY: `t.io_base` was validated during init; reading the legacy ISR
    // register acknowledges a pending interrupt line.
    let _ = unsafe { inb(t.io_base + VIRTIO_PCI_ISR) };
}

pub fn receive_into(out: &mut [u8]) -> Option<usize> {
    let mut t = TRANSPORT.lock();
    if !t.initialized {
        return None;
    }
    let mem = dma_ptr(RX_QUEUE_OFFSET);
    // SAFETY: `mem` is the RX queue sub-region of the DMA reservation, so
    // `used_idx_ptr` lands within it; accessed under the TRANSPORT lock.
    let used_idx = unsafe { read_u16(used_idx_ptr(mem, t.rx_queue_size)) };
    if used_idx == t.rx_last_used {
        return None;
    }

    fence(Ordering::Acquire);
    let ring_slot = (t.rx_last_used as usize) % t.rx_queue_size as usize;
    // SAFETY: `ring_slot < rx_queue_size`, so `used_elem_ptr` addresses a
    // VirtqUsedElem inside the RX used ring; the Acquire fence orders this read
    // after the device's write of the element.
    let elem = unsafe { core::ptr::read_volatile(used_elem_ptr(mem, t.rx_queue_size, ring_slot)) };
    t.rx_last_used = t.rx_last_used.wrapping_add(1);

    let id = elem.id as usize;
    if id >= ACTIVE_RX_DESCRIPTORS {
        t.rx_dropped = t.rx_dropped.saturating_add(1);
        return None;
    }
    if elem.len as usize <= VIRTIO_NET_HDR_BYTES {
        t.rx_dropped = t.rx_dropped.saturating_add(1);
        repost_rx(&mut t, id as u16);
        return None;
    }
    let frame_len = core::cmp::min(elem.len as usize - VIRTIO_NET_HDR_BYTES, MAX_FRAME);
    if out.len() < frame_len {
        t.rx_dropped = t.rx_dropped.saturating_add(1);
        repost_rx(&mut t, id as u16);
        return None;
    }
    let packet = dma_ptr(RX_PACKETS_OFFSET + id * PACKET_BYTES) as *const u8;
    // SAFETY: `id < ACTIVE_RX_DESCRIPTORS`, so `packet` is that descriptor's
    // 2048-byte RX buffer in the DMA region; `frame_len` was clamped to both the
    // buffer payload and `out.len()`, so source and destination are each valid
    // for `frame_len` bytes and the kernel buffer cannot overlap the caller's.
    unsafe {
        core::ptr::copy_nonoverlapping(
            packet.add(VIRTIO_NET_HDR_BYTES),
            out.as_mut_ptr(),
            frame_len,
        );
    }
    t.rx_frames = t.rx_frames.saturating_add(1);
    repost_rx(&mut t, id as u16);
    Some(frame_len)
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

    let packet = dma_ptr(TX_PACKET_OFFSET);
    // SAFETY: `packet` is the single PACKET_BYTES-long TX buffer in the DMA
    // region; `frame.len() <= MAX_FRAME` was checked above, so the 10-byte
    // header zero-fill and the frame copy both stay within it, and `frame` is a
    // caller slice that cannot overlap the kernel DMA region.
    unsafe {
        core::ptr::write_bytes(packet, 0, VIRTIO_NET_HDR_BYTES);
        core::ptr::copy_nonoverlapping(
            frame.as_ptr(),
            packet.add(VIRTIO_NET_HDR_BYTES),
            frame.len(),
        );
    }
    let mem = dma_ptr(TX_QUEUE_OFFSET);
    // SAFETY: `mem` is the TX queue region and descriptor 0 is the sole TX
    // descriptor within it. We hold the TRANSPORT lock and `tx_outstanding` is
    // false, so the device does not own it and no other reference aliases it.
    let desc = unsafe { &mut *desc_ptr(mem, 0) };
    desc.len = (VIRTIO_NET_HDR_BYTES + frame.len()) as u32;
    desc.flags = 0;
    desc.next = 0;

    // SAFETY: `mem`/`t.tx_queue_size` describe the TX queue and descriptor 0 is
    // valid (see push_avail's contract).
    unsafe {
        push_avail(mem, t.tx_queue_size, 0);
    }
    fence(Ordering::SeqCst);
    // SAFETY: QUEUE_NOTIFY write on the validated I/O BAR kicks the TX queue.
    unsafe {
        outw(t.io_base + VIRTIO_PCI_QUEUE_NOTIFY, TX_QUEUE);
    }
    t.tx_outstanding = true;
    t.tx_frames = t.tx_frames.saturating_add(1);
    true
}

fn setup_queue(io_base: u16, queue: u16) -> Result<u16, InitError> {
    // SAFETY: selecting a queue via QUEUE_SEL on the validated I/O BAR.
    unsafe {
        outw(io_base + VIRTIO_PCI_QUEUE_SEL, queue);
    }
    // SAFETY: reading the selected queue's size via QUEUE_NUM on the same BAR.
    let size = unsafe { inw(io_base + VIRTIO_PCI_QUEUE_NUM) };
    if size == 0 {
        fail(io_base);
        return Err(InitError::QueueUnavailable);
    }
    if size as usize > MAX_QUEUE_SIZE {
        fail(io_base);
        return Err(InitError::QueueTooLarge);
    }

    let total = queue_total_bytes(size);
    if total > QUEUE_MEMORY_BYTES {
        fail(io_base);
        return Err(InitError::QueueTooLarge);
    }
    let offset = if queue == RX_QUEUE {
        RX_QUEUE_OFFSET
    } else {
        TX_QUEUE_OFFSET
    };
    let ptr = dma_ptr(offset);
    // SAFETY: `ptr` is the start of this queue's QUEUE_MEMORY_BYTES sub-region
    // in the DMA reservation (`offset` is RX_QUEUE_OFFSET or TX_QUEUE_OFFSET),
    // so zeroing exactly that span stays in bounds.
    unsafe {
        core::ptr::write_bytes(ptr, 0, QUEUE_MEMORY_BYTES);
    }
    let phys = dma_phys(offset);
    if phys & (PAGE_SIZE as u64 - 1) != 0 {
        fail(io_base);
        return Err(InitError::DmaNotContiguous);
    }
    // SAFETY: programming the queue's page-frame number into QUEUE_PFN on the
    // validated I/O BAR; `phys` is page-aligned (checked just above) and names
    // the zeroed queue region.
    unsafe {
        outl(io_base + VIRTIO_PCI_QUEUE_PFN, (phys >> 12) as u32);
    }
    Ok(size)
}

fn post_initial_rx(io_base: u16, qsize: u16) -> Result<(), InitError> {
    let mem = dma_ptr(RX_QUEUE_OFFSET);
    for id in 0..ACTIVE_RX_DESCRIPTORS {
        let packet_offset = RX_PACKETS_OFFSET + id * PACKET_BYTES;
        let phys = dma_phys(packet_offset);
        // SAFETY: `id < ACTIVE_RX_DESCRIPTORS <= qsize`, so `desc_ptr` lands on
        // RX descriptor `id` within the queue region; init() runs
        // single-threaded here and the device is not yet started.
        let desc = unsafe { &mut *desc_ptr(mem, id) };
        *desc = VirtqDesc {
            addr: phys,
            len: PACKET_BYTES as u32,
            flags: DESC_F_WRITE,
            next: 0,
        };
        // SAFETY: `mem`/`qsize` describe the RX queue and `id < qsize`.
        unsafe {
            push_avail(mem, qsize, id as u16);
        }
    }
    fence(Ordering::SeqCst);
    // SAFETY: notify the RX queue on the validated I/O BAR.
    unsafe {
        outw(io_base + VIRTIO_PCI_QUEUE_NOTIFY, RX_QUEUE);
    }
    Ok(())
}

fn setup_tx_descriptor(_qsize: u16) -> Result<(), InitError> {
    let mem = dma_ptr(TX_QUEUE_OFFSET);
    let phys = dma_phys(TX_PACKET_OFFSET);
    // SAFETY: `mem` is the TX queue region; descriptor 0 lies within it and is
    // written only here during single-threaded init.
    unsafe {
        *desc_ptr(mem, 0) = VirtqDesc {
            addr: phys,
            len: 0,
            flags: 0,
            next: 0,
        };
    }
    Ok(())
}

fn repost_rx(t: &mut Transport, id: u16) {
    let mem = dma_ptr(RX_QUEUE_OFFSET);
    // SAFETY: `mem`/`t.rx_queue_size` describe the RX queue and `id` is a
    // descriptor index returned by the device (< queue size).
    unsafe {
        push_avail(mem, t.rx_queue_size, id);
    }
    fence(Ordering::SeqCst);
    // SAFETY: notify the RX queue on the validated I/O BAR.
    unsafe {
        outw(t.io_base + VIRTIO_PCI_QUEUE_NOTIFY, RX_QUEUE);
    }
}

fn reap_tx(t: &mut Transport) {
    if !t.tx_outstanding {
        return;
    }
    let mem = dma_ptr(TX_QUEUE_OFFSET);
    // SAFETY: `mem` is the TX queue region; `used_idx_ptr` lands in its used
    // ring. Called under the TRANSPORT lock.
    let used_idx = unsafe { read_u16(used_idx_ptr(mem, t.tx_queue_size)) };
    if used_idx != t.tx_last_used {
        fence(Ordering::Acquire);
        t.tx_last_used = used_idx;
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

const fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}
const fn used_offset(qsize: u16) -> usize {
    align_up(
        size_of::<VirtqDesc>() * qsize as usize + 6 + 2 * qsize as usize,
        PAGE_SIZE,
    )
}
const fn queue_total_bytes(qsize: u16) -> usize {
    used_offset(qsize) + 6 + size_of::<VirtqUsedElem>() * qsize as usize
}

/// # Safety
/// `mem` must point at the base of a virtqueue region within the DMA
/// reservation and `id` must be a valid descriptor index (`id < qsize`), so the
/// returned pointer stays inside that queue's descriptor table.
unsafe fn desc_ptr(mem: *mut u8, id: usize) -> *mut VirtqDesc {
    // SAFETY: by the contract above `id * size_of::<VirtqDesc>()` is within the
    // descriptor table, so the offset pointer is in bounds.
    unsafe { mem.add(id * size_of::<VirtqDesc>()) as *mut VirtqDesc }
}
/// # Safety
/// `mem`/`qsize` must describe a virtqueue region in the DMA reservation laid
/// out by [`queue_total_bytes`].
unsafe fn avail_idx_ptr(mem: *mut u8, qsize: u16) -> *mut u16 {
    // SAFETY: the available-ring index sits at `desc_table + 2` within the
    // queue region described by the contract above.
    unsafe { mem.add(size_of::<VirtqDesc>() * qsize as usize + 2) as *mut u16 }
}
/// # Safety
/// `mem`/`qsize` must describe a virtqueue region and `slot < qsize`.
unsafe fn avail_ring_ptr(mem: *mut u8, qsize: u16, slot: usize) -> *mut u16 {
    // SAFETY: entry `slot` (`slot < qsize`) of the available ring lies at
    // `desc_table + 4 + slot*2`, inside the queue region.
    unsafe { mem.add(size_of::<VirtqDesc>() * qsize as usize + 4 + slot * 2) as *mut u16 }
}
/// # Safety
/// `mem`/`qsize` must describe a virtqueue region in the DMA reservation.
unsafe fn used_idx_ptr(mem: *mut u8, qsize: u16) -> *mut u16 {
    // SAFETY: the used-ring index sits at `used_offset(qsize) + 2`, inside the
    // queue region.
    unsafe { mem.add(used_offset(qsize) + 2) as *mut u16 }
}
/// # Safety
/// `mem`/`qsize` must describe a virtqueue region and `slot < qsize`.
unsafe fn used_elem_ptr(mem: *mut u8, qsize: u16, slot: usize) -> *mut VirtqUsedElem {
    // SAFETY: used element `slot` (`slot < qsize`) lies at
    // `used_offset(qsize) + 4 + slot*size_of::<VirtqUsedElem>()`, inside the
    // queue region whose total size is bounded by `queue_total_bytes`.
    unsafe {
        mem.add(used_offset(qsize) + 4 + slot * size_of::<VirtqUsedElem>()) as *mut VirtqUsedElem
    }
}

/// # Safety
/// `mem`/`qsize` must describe a virtqueue region in the DMA reservation and
/// `descriptor` must be a valid descriptor index for it. Call under the
/// `TRANSPORT` lock so the available ring has a single writer.
unsafe fn push_avail(mem: *mut u8, qsize: u16, descriptor: u16) {
    // SAFETY: the sub-pointers are derived from the same valid queue region and
    // `slot < qsize`; the Release fence publishes the ring entry before the
    // index bump so the device observes a consistent available ring.
    let idx_ptr = unsafe { avail_idx_ptr(mem, qsize) };
    let idx = unsafe { read_u16(idx_ptr) };
    let slot = idx as usize % qsize as usize;
    // SAFETY: `slot < qsize`, so `avail_ring_ptr` is in bounds of the ring.
    unsafe {
        core::ptr::write_volatile(avail_ring_ptr(mem, qsize, slot), descriptor);
    }
    fence(Ordering::Release);
    // SAFETY: `idx_ptr` is the available-ring index within the same queue; the
    // Release fence above published the entry before this index bump.
    unsafe {
        core::ptr::write_volatile(idx_ptr, idx.wrapping_add(1));
    }
}
/// # Safety
/// `ptr` must be valid and aligned for a `u16` read (ring indices live in the
/// DMA reservation, which satisfies both).
unsafe fn read_u16(ptr: *mut u16) -> u16 {
    // SAFETY: guaranteed valid/aligned by the contract above; volatile because
    // the device writes the same location.
    unsafe { core::ptr::read_volatile(ptr) }
}

fn fail(io_base: u16) {
    // SAFETY: `io_base` is this device's validated I/O BAR base; writing
    // STATUS_FAILED to the legacy status register signals driver failure.
    unsafe {
        outb(io_base + VIRTIO_PCI_STATUS, STATUS_FAILED);
    }
}

pub fn self_test() -> bool {
    size_of::<VirtqDesc>() == 16
        && size_of::<VirtqUsedElem>() == 8
        && queue_total_bytes(256) <= QUEUE_MEMORY_BYTES
        && ACTIVE_RX_DESCRIPTORS <= MAX_QUEUE_SIZE
}

/// # Safety
/// `port` must be a valid x86 I/O port this driver is permitted to access
/// (callers pass `io_base + <known virtio register offset>`). Port reads can
/// have device-visible side effects.
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: `in al, dx` reads one byte from the I/O port in `dx`; sound for a
    // valid port per the contract above. `nomem` is correct as it touches no
    // memory.
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    }
    value
}
/// # Safety
/// See [`inb`]: `port` must be a valid I/O port.
unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    // SAFETY: `in ax, dx` reads one word from the valid I/O port in `dx`.
    unsafe {
        core::arch::asm!("in ax, dx", in("dx") port, out("ax") value, options(nomem, nostack, preserves_flags));
    }
    value
}
/// # Safety
/// See [`inb`]: `port` must be a valid I/O port.
unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    // SAFETY: `in eax, dx` reads one dword from the valid I/O port in `dx`.
    unsafe {
        core::arch::asm!("in eax, dx", in("dx") port, out("eax") value, options(nomem, nostack, preserves_flags));
    }
    value
}
/// # Safety
/// See [`inb`]: `port` must be a valid I/O port; the write may have
/// device-visible side effects.
unsafe fn outb(port: u16, value: u8) {
    // SAFETY: `out dx, al` writes one byte to the valid I/O port in `dx`.
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}
/// # Safety
/// See [`inb`]: `port` must be a valid I/O port.
unsafe fn outw(port: u16, value: u16) {
    // SAFETY: `out dx, ax` writes one word to the valid I/O port in `dx`.
    unsafe {
        core::arch::asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags));
    }
}
/// # Safety
/// See [`inb`]: `port` must be a valid I/O port.
unsafe fn outl(port: u16, value: u32) {
    // SAFETY: `out dx, eax` writes one dword to the valid I/O port in `dx`.
    unsafe {
        core::arch::asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags));
    }
}
