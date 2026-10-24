#![cfg_attr(
    any(not(feature = "qemu-test"), feature = "stage13-12-test"),
    expect(
        dead_code,
        reason = "production initializes the bound controller (init_at); HID and Bluetooth paths are exercised by the Stage 13.5/13.6/13.10 gates until the Stage 13 USB input step gives them production callers"
    )
)]
use core::{
    cmp, ptr,
    sync::atomic::{fence, AtomicU64, AtomicU8, Ordering},
};

use crate::{
    hal::pci::{self, Address, BarKind},
    irq_lock::IrqMutex as Mutex,
    memory, paging,
    woven_input::{Event, Key},
};

const USB_CLASS: u8 = 0x0c;
const USB_SUBCLASS: u8 = 0x03;
const XHCI_PROG_IF: u8 = 0x30;
const PAGE_SIZE: usize = 4096;
const POLL_LIMIT: usize = 4_000_000;
const MAX_EVENTS_PER_WAIT: usize = 64;

const TRB_TYPE_NORMAL: u32 = 1;
const TRB_TYPE_SETUP_STAGE: u32 = 2;
const TRB_TYPE_DATA_STAGE: u32 = 3;
const TRB_TYPE_STATUS_STAGE: u32 = 4;
const TRB_TYPE_ENABLE_SLOT: u32 = 9;
const TRB_TYPE_ADDRESS_DEVICE: u32 = 11;
const TRB_TYPE_CONFIGURE_ENDPOINT: u32 = 12;
const TRB_TYPE_TRANSFER_EVENT: u32 = 32;
const TRB_TYPE_COMMAND_COMPLETION: u32 = 33;
const TRB_TYPE_PORT_STATUS_CHANGE: u32 = 34;

const USB_REQUEST_GET_DESCRIPTOR: u8 = 6;
const USB_REQUEST_SET_CONFIGURATION: u8 = 9;
const USB_REQUEST_SET_IDLE: u8 = 10;
const USB_REQUEST_SET_PROTOCOL: u8 = 11;
const USB_DESCRIPTOR_DEVICE: u8 = 1;
const USB_DESCRIPTOR_CONFIGURATION: u8 = 2;
const USB_CLASS_HID: u8 = 3;
const USB_CLASS_WIRELESS_CONTROLLER: u8 = 0xe0;
const USB_SUBCLASS_RF_CONTROLLER: u8 = 0x01;
const USB_PROTOCOL_BLUETOOTH_PRIMARY: u8 = 0x01;
const USB_ENDPOINT_BULK: u8 = 0x02;
const USB_ENDPOINT_INTERRUPT: u8 = 0x03;
const USB_REQUEST_TYPE_BLUETOOTH_COMMAND: u8 = 0x20;
const USB_PROTOCOL_KEYBOARD: u8 = 1;
const USB_PROTOCOL_MOUSE: u8 = 2;

static CONTROLLER: Mutex<Option<XhciController>> = Mutex::with_rank(None, 10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitError {
    MissingController,
    MissingMemoryBar,
    DmaUnavailable,
    MmioUnavailable,
    ControllerTimeout,
    CommandFailed,
    TransferFailed,
    NoConnectedPort,
    InvalidRegisters,
    DescriptorInvalid,
    HidNotFound,
    BluetoothNotFound,
    /// No MSI/MSI-X vector could be bound for input delivery.
    InterruptUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HidKind {
    Keyboard,
    Mouse,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HidSummary {
    pub slot: u8,
    pub port: u8,
    pub interface: u8,
    pub endpoint: u8,
    pub max_packet: u16,
    pub kind: HidKind,
}

#[derive(Clone, Copy)]
struct DmaPage {
    physical: u64,
    virtual_address: u64,
}

impl DmaPage {
    fn allocate_zeroed() -> Option<Self> {
        let frame = memory::allocate_contiguous_frames(1)?;
        let physical = frame.start_address().as_u64();
        let offset = paging::physical_memory_offset()?;
        let virtual_address = offset.checked_add(physical)?;
        // SAFETY: the allocated frame is exclusively owned by this DMA object.
        unsafe { ptr::write_bytes(virtual_address as *mut u8, 0, PAGE_SIZE) };
        Some(Self {
            physical,
            virtual_address,
        })
    }

    fn bytes(&self) -> &[u8; PAGE_SIZE] {
        // SAFETY: DMA pages remain allocated for the lifetime of the controller.
        unsafe { &*(self.virtual_address as *const [u8; PAGE_SIZE]) }
    }

    fn bytes_mut(&mut self) -> &mut [u8; PAGE_SIZE] {
        // SAFETY: all mutable access is serialized by the controller mutex.
        unsafe { &mut *(self.virtual_address as *mut [u8; PAGE_SIZE]) }
    }
}

#[derive(Clone, Copy, Default)]
struct Trb {
    parameter: u64,
    status: u32,
    control: u32,
}

struct Ring {
    page: DmaPage,
    index: usize,
    cycle: bool,
}

impl Ring {
    fn new(mut page: DmaPage) -> Self {
        page.bytes_mut().fill(0);
        let mut ring = Self {
            page,
            index: 0,
            cycle: true,
        };
        ring.install_link();
        ring
    }

    fn install_link(&mut self) {
        let slot = (PAGE_SIZE / 16) - 1;
        let mut trb = Trb {
            parameter: self.page.physical,
            status: 0,
            control: (6 << 10) | (1 << 1),
        };
        if self.cycle {
            trb.control |= 1;
        }
        self.write(slot, trb);
    }

    fn push(&mut self, mut trb: Trb) -> u64 {
        if self.index == (PAGE_SIZE / 16) - 1 {
            // The link TRB carries the cycle the consumer is on, so it follows
            // it back to the start; only then does the producer's cycle flip.
            // Written after the flip, it would carry the new cycle, and a
            // controller that had not yet fetched it would stop there.
            self.install_link();
            self.index = 0;
            self.cycle = !self.cycle;
        }
        if self.cycle {
            trb.control |= 1;
        } else {
            trb.control &= !1;
        }
        let index = self.index;
        self.write(index, trb);
        fence(Ordering::Release);
        self.index += 1;
        self.page.physical + (index as u64 * 16)
    }

    fn write(&mut self, index: usize, trb: Trb) {
        let offset = index * 16;
        let bytes = self.page.bytes_mut();
        bytes[offset..offset + 8].copy_from_slice(&trb.parameter.to_le_bytes());
        bytes[offset + 8..offset + 12].copy_from_slice(&trb.status.to_le_bytes());
        bytes[offset + 12..offset + 16].copy_from_slice(&trb.control.to_le_bytes());
    }
}

/// Keyboards and mice the controller delivers into WovenInput.
const MAX_INPUTS: usize = 4;
/// Reports copied out per pass over the event ring.
const REPORTS_PER_PASS: usize = 32;
/// The longest report kept; longer ones are truncated.
const MAX_REPORT: usize = 64;

/// One enumerated USB keyboard or mouse, with its own slot, rings and
/// report buffer; one interrupt transfer is always armed for it.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
struct InputDevice {
    slot_id: u8,
    port: u8,
    kind: HidKind,
    /// Report-protocol devices (tablets, digitizers, game controllers): the
    /// parsed report descriptor.
    layout: Option<crate::hid::Layout>,
    dci: u8,
    length: usize,
    ring: Ring,
    buffer: DmaPage,
    /// Physical address of the armed Normal TRB.
    pending: u64,
    _output_context: Option<DmaPage>,
    _ep0_ring: Option<Ring>,
}

/// Input reports kept while a storage transfer waits (see `stashed`).
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
const STASHED_REPORTS: usize = 32;
/// Transfer Request Block types the USB disk's stall recovery uses.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
const TRB_TYPE_RESET_ENDPOINT: u32 = 14;
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
const TRB_TYPE_SET_TR_DEQUEUE: u32 = 16;
/// Completion codes: success, stall, short packet.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
const COMPLETION_STALL: u8 = 6;
/// TEST UNIT READY attempts while a disk becomes ready.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
const READY_ATTEMPTS: usize = 10;

/// The USB disk: a Bulk-Only SCSI device with its own slot, one ring per
/// bulk endpoint, a data page and a page for the command and status
/// wrappers. One command is in flight at a time, under the controller lock.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
struct StorageDevice {
    slot_id: u8,
    port: u8,
    interface: crate::usb_msc::Interface,
    in_dci: u8,
    out_dci: u8,
    in_ring: Ring,
    out_ring: Ring,
    data: DmaPage,
    wrappers: DmaPage,
    tag: u32,
    blocks: u64,
    block_size: u32,
    vendor: [u8; 8],
    product: [u8; 16],
    /// Kept for control transfers (stall recovery) and so the slot's
    /// contexts outlive enumeration.
    output_context: Option<DmaPage>,
    ep0_ring: Option<Ring>,
}

/// Why a USB disk command failed.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    /// The device reported CHECK CONDITION; REQUEST SENSE says why.
    CheckCondition,
    /// A transfer stalled or failed, or the status wrapper was invalid.
    Transport,
    /// The controller did not complete the transfer in time.
    Timeout,
    /// No disk, or the request does not fit it.
    NoDisk,
}

#[derive(Clone, Copy)]
struct HidInterface {
    interface: u8,
    configuration: u8,
    endpoint_address: u8,
    max_packet: u16,
    interval: u8,
    protocol: u8,
    /// Length of the interface's report descriptor, from its HID descriptor.
    report_length: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BluetoothUsbInterface {
    pub interface: u8,
    pub configuration: u8,
    pub event_endpoint: u8,
    pub event_max_packet: u16,
    pub event_interval: u8,
    pub acl_in_endpoint: u8,
    pub acl_in_max_packet: u16,
    pub acl_out_endpoint: u8,
    pub acl_out_max_packet: u16,
}

pub struct XhciController {
    address: Address,
    original_command: u16,
    max_slots: u8,
    max_ports: u8,
    op_base: u64,
    runtime_base: u64,
    doorbell_base: u64,
    context_size: usize,
    dcbaa: DmaPage,
    command_ring: Ring,
    event_ring: DmaPage,
    _erst: DmaPage,
    event_index: usize,
    event_cycle: bool,
    connected_port: u8,
    slot_id: u8,
    output_context: Option<DmaPage>,
    ep0_ring: Option<Ring>,
    hid_ring: Option<Ring>,
    hid_buffer: Option<DmaPage>,
    hid: Option<HidInterface>,
    bluetooth: Option<BluetoothUsbInterface>,
    bt_event_ring: Option<Ring>,
    bt_event_buffer: Option<DmaPage>,
    bt_acl_in_ring: Option<Ring>,
    bt_acl_in_buffer: Option<DmaPage>,
    bt_acl_out_ring: Option<Ring>,
    bt_acl_out_buffer: Option<DmaPage>,
    #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
    inputs: [Option<InputDevice>; MAX_INPUTS],
    /// The USB disk (Stage 13.18).
    #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
    storage: Option<StorageDevice>,
    /// Input reports a storage transfer drained while waiting for its own
    /// completion, kept for the input worker's next pass.
    #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
    stashed: [(HidKind, usize, [u8; MAX_REPORT]); STASHED_REPORTS],
    #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
    stashed_len: usize,
}

impl XhciController {
    fn initialize(device: pci::Device) -> Result<Self, InitError> {
        crate::serial::write_line(format_args!("[XHCI] controller initialization begin"));
        let bar = device
            .bars
            .iter()
            .find(|bar| bar.valid && matches!(bar.kind, BarKind::Memory32 | BarKind::Memory64))
            .ok_or(InitError::MissingMemoryBar)?;
        let address = Address {
            segment: device.segment,
            bus: device.bus,
            device: device.device,
            function: device.function,
        };
        if !pci::enable_memory_bus_master(address) {
            return Err(InitError::MissingController);
        }
        crate::serial::write_line(format_args!("[XHCI] PCI memory and bus mastering enabled"));
        let mmio = bar.address;
        let cap_length = mmio_read8(mmio)?;
        if cap_length < 0x20 {
            return Err(InitError::InvalidRegisters);
        }
        let hcs1 = mmio_read32(mmio + 0x04)?;
        let hcc1 = mmio_read32(mmio + 0x10)?;
        let max_slots = (hcs1 & 0xff) as u8;
        let max_ports = ((hcs1 >> 24) & 0xff) as u8;
        if max_slots == 0 || max_ports == 0 {
            return Err(InitError::InvalidRegisters);
        }
        let context_size = if hcc1 & (1 << 2) != 0 { 64 } else { 32 };
        let runtime_base = mmio + u64::from(mmio_read32(mmio + 0x18)? & !0x1f);
        let doorbell_base = mmio + u64::from(mmio_read32(mmio + 0x14)? & !0x3);
        let op_base = mmio + u64::from(cap_length);

        let command = mmio_read32(op_base)?;
        mmio_write32(op_base, command & !1)?;
        crate::serial::write_line(format_args!("[XHCI] controller halt requested"));
        wait_until(op_base + 0x04, 1, 1)?;
        mmio_write32(op_base, (command & !1) | (1 << 1))?;
        crate::serial::write_line(format_args!("[XHCI] controller reset requested"));
        wait_until(op_base, 1 << 1, 0)?;
        crate::serial::write_line(format_args!("[XHCI] controller reset complete"));

        let mut dcbaa = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let command_page = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let mut event_ring = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let mut erst = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        dcbaa.bytes_mut().fill(0);
        event_ring.bytes_mut().fill(0);
        erst.bytes_mut().fill(0);
        let command_ring = Ring::new(command_page);

        mmio_write64(op_base + 0x30, dcbaa.physical)?;
        mmio_write64(op_base + 0x18, command_ring.page.physical | 1)?;
        mmio_write32(op_base + 0x38, u32::from(max_slots))?;

        let interrupter = runtime_base + 0x20;
        // Stage 13.6 deliberately keeps completion delivery polling-based until
        // the generic MSI/MSI-X allocator is available. Keep the interrupter
        // disabled while still programming its event-ring state.
        mmio_write32(interrupter, 0)?;
        mmio_write32(interrupter + 0x08, 1)?;
        let erst_bytes = erst.bytes_mut();
        erst_bytes[0..8].copy_from_slice(&event_ring.physical.to_le_bytes());
        erst_bytes[8..12].copy_from_slice(&((PAGE_SIZE / 16) as u32).to_le_bytes());
        mmio_write64(interrupter + 0x10, erst.physical)?;
        mmio_write64(interrupter + 0x18, event_ring.physical)?;

        mmio_write32(op_base, 1)?;
        crate::serial::write_line(format_args!("[XHCI] controller start requested"));
        wait_until(op_base + 0x04, 1, 0)?;

        let connected_port = find_connected_port(op_base, max_ports)?;
        crate::serial::write_line(format_args!(
            "[XHCI] connected port {} found",
            connected_port
        ));
        let mut controller = Self {
            address,
            original_command: device.command,
            max_slots,
            max_ports,
            op_base,
            runtime_base,
            doorbell_base,
            context_size,
            dcbaa,
            command_ring,
            event_ring,
            _erst: erst,
            event_index: 0,
            event_cycle: true,
            connected_port,
            slot_id: 0,
            output_context: None,
            ep0_ring: None,
            hid_ring: None,
            hid_buffer: None,
            hid: None,
            bluetooth: None,
            bt_event_ring: None,
            bt_event_buffer: None,
            bt_acl_in_ring: None,
            bt_acl_in_buffer: None,
            bt_acl_out_ring: None,
            bt_acl_out_buffer: None,
            #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
            inputs: [const { None }; MAX_INPUTS],
            #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
            storage: None,
            #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
            stashed: [(HidKind::Other, 0, [0; MAX_REPORT]); STASHED_REPORTS],
            #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
            stashed_len: 0,
        };
        controller.reset_port(connected_port)?;
        crate::serial::write_line(format_args!(
            "[XHCI] port {} reset complete",
            connected_port
        ));
        controller.slot_id = controller.enable_slot()?;
        crate::serial::write_line(format_args!("[XHCI] initial slot enabled"));
        Ok(controller)
    }

    /// Disable xHCI interrupt generation and halt the controller before any
    /// PCI BAR or DMA-page ownership is released.
    fn shutdown_hardware(&mut self) -> Result<(), InitError> {
        let interrupter = self.runtime_base + 0x20;
        mmio_write32(interrupter, 0)?;
        let command = mmio_read32(self.op_base)?;
        mmio_write32(self.op_base, command & !(1 | (1 << 2)))?;
        wait_until(self.op_base + 0x04, 1, 1)?;
        if !pci::restore_command(self.address, self.original_command) {
            return Err(InitError::MissingController);
        }
        Ok(())
    }

    fn reset_port(&mut self, port: u8) -> Result<(), InitError> {
        let portsc = self.op_base + 0x400 + (u64::from(port - 1) * 0x10);
        let value = mmio_read32(portsc)?;
        mmio_write32(portsc, value | (1 << 4))?;
        for _ in 0..POLL_LIMIT {
            let current = mmio_read32(portsc)?;
            if current & (1 << 4) == 0 && current & 1 != 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(InitError::ControllerTimeout)
    }

    fn port_speed(&self) -> Result<u8, InitError> {
        let portsc = self.op_base + 0x400 + (u64::from(self.connected_port - 1) * 0x10);
        Ok(((mmio_read32(portsc)? >> 10) & 0x0f) as u8)
    }

    fn enable_slot(&mut self) -> Result<u8, InitError> {
        let command_ptr = self.command_ring.push(Trb {
            parameter: 0,
            status: 0,
            control: TRB_TYPE_ENABLE_SLOT << 10,
        });
        mmio_write32(self.doorbell_base, 0)?;
        let event = self.wait_command_completion(command_ptr)?;
        let slot = ((event.control >> 24) & 0xff) as u8;
        if slot == 0 {
            return Err(InitError::CommandFailed);
        }
        Ok(slot)
    }

    fn address_device(&mut self) -> Result<(), InitError> {
        let speed = self.port_speed()?;
        let ep0_max_packet: u32 = match speed {
            3 => 64,
            4 | 5 => 512,
            _ => 8,
        };
        let mut output = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let ep0_page = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let ep0_ring = Ring::new(ep0_page);
        let mut input = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        output.bytes_mut().fill(0);
        input.bytes_mut().fill(0);

        write_u64(
            self.dcbaa.bytes_mut(),
            usize::from(self.slot_id) * 8,
            output.physical,
        );
        write_u32(input.bytes_mut(), 4, 0x3); // add Slot + EP0 contexts
        let slot_offset = self.context_size;
        write_u32(
            input.bytes_mut(),
            slot_offset,
            (u32::from(speed) << 20) | (1 << 27),
        );
        write_u32(
            input.bytes_mut(),
            slot_offset + 4,
            u32::from(self.connected_port) << 16,
        );
        let ep0_offset = self.context_size * 2;
        write_u32(
            input.bytes_mut(),
            ep0_offset + 4,
            (3 << 1) | (4 << 3) | (ep0_max_packet << 16),
        );
        write_u64(
            input.bytes_mut(),
            ep0_offset + 8,
            ep0_ring.page.physical | 1,
        );
        write_u32(input.bytes_mut(), ep0_offset + 16, 8);
        fence(Ordering::Release);

        let command_ptr = self.command_ring.push(Trb {
            parameter: input.physical,
            status: 0,
            control: (TRB_TYPE_ADDRESS_DEVICE << 10) | (u32::from(self.slot_id) << 24),
        });
        mmio_write32(self.doorbell_base, 0)?;
        self.wait_command_completion(command_ptr)?;
        self.output_context = Some(output);
        self.ep0_ring = Some(ep0_ring);
        Ok(())
    }

    fn enumerate_hid(&mut self) -> Result<HidSummary, InitError> {
        self.address_device()?;
        let (descriptor, total_length, configuration) = self.read_configuration()?;
        self.finish_hid(&descriptor, total_length, configuration)
    }

    /// Check the addressed device's device descriptor, then read its whole
    /// configuration descriptor: (buffer, length, configuration value).
    fn read_configuration(&mut self) -> Result<(DmaPage, usize, u8), InitError> {
        let mut descriptor = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        self.control_in(
            0x80,
            USB_REQUEST_GET_DESCRIPTOR,
            u16::from(USB_DESCRIPTOR_DEVICE) << 8,
            0,
            &mut descriptor,
            18,
        )?;
        let device = descriptor.bytes();
        if device[0] < 18 || device[1] != USB_DESCRIPTOR_DEVICE {
            return Err(InitError::DescriptorInvalid);
        }

        self.control_in(
            0x80,
            USB_REQUEST_GET_DESCRIPTOR,
            u16::from(USB_DESCRIPTOR_CONFIGURATION) << 8,
            0,
            &mut descriptor,
            9,
        )?;
        let header = descriptor.bytes();
        if header[0] < 9 || header[1] != USB_DESCRIPTOR_CONFIGURATION {
            return Err(InitError::DescriptorInvalid);
        }
        let total_length = usize::from(u16::from_le_bytes([header[2], header[3]]));
        if !(9..=PAGE_SIZE).contains(&total_length) {
            return Err(InitError::DescriptorInvalid);
        }
        let configuration = header[5];
        self.control_in(
            0x80,
            USB_REQUEST_GET_DESCRIPTOR,
            u16::from(USB_DESCRIPTOR_CONFIGURATION) << 8,
            0,
            &mut descriptor,
            total_length,
        )?;
        Ok((descriptor, total_length, configuration))
    }

    /// Configure the addressed device as a HID device from its configuration.
    fn finish_hid(
        &mut self,
        descriptor: &DmaPage,
        total_length: usize,
        configuration: u8,
    ) -> Result<HidSummary, InitError> {
        let hid = parse_hid_interface(&descriptor.bytes()[..total_length], configuration)
            .ok_or(InitError::HidNotFound)?;

        self.control_no_data(
            0x00,
            USB_REQUEST_SET_CONFIGURATION,
            u16::from(hid.configuration),
            0,
        )?;
        if hid.protocol == USB_PROTOCOL_KEYBOARD || hid.protocol == USB_PROTOCOL_MOUSE {
            // HID Set Protocol: wValue=0 selects the fixed boot report format.
            self.control_no_data(0x21, USB_REQUEST_SET_PROTOCOL, 0, u16::from(hid.interface))?;
            // A small non-zero idle period makes an unchanged boot report observable
            // on QEMU's interrupt endpoint without synthesizing host key input.
            self.control_no_data(0x21, USB_REQUEST_SET_IDLE, 1 << 8, u16::from(hid.interface))?;
        }
        self.configure_hid_endpoint(hid)?;
        self.hid = Some(hid);

        Ok(HidSummary {
            slot: self.slot_id,
            port: self.connected_port,
            interface: hid.interface,
            endpoint: hid.endpoint_address,
            max_packet: hid.max_packet,
            kind: hid_kind(hid.protocol),
        })
    }

    fn enumerate_bluetooth(&mut self) -> Result<BluetoothUsbInterface, InitError> {
        self.address_device()?;
        let mut descriptor = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        self.control_in(
            0x80,
            USB_REQUEST_GET_DESCRIPTOR,
            u16::from(USB_DESCRIPTOR_CONFIGURATION) << 8,
            0,
            &mut descriptor,
            9,
        )?;
        let header = descriptor.bytes();
        if header[0] < 9 || header[1] != USB_DESCRIPTOR_CONFIGURATION {
            return Err(InitError::DescriptorInvalid);
        }
        let total_length = usize::from(u16::from_le_bytes([header[2], header[3]]));
        if !(9..=PAGE_SIZE).contains(&total_length) {
            return Err(InitError::DescriptorInvalid);
        }
        let configuration = header[5];
        self.control_in(
            0x80,
            USB_REQUEST_GET_DESCRIPTOR,
            u16::from(USB_DESCRIPTOR_CONFIGURATION) << 8,
            0,
            &mut descriptor,
            total_length,
        )?;
        let bluetooth =
            parse_bluetooth_usb_interface(&descriptor.bytes()[..total_length], configuration)
                .ok_or(InitError::BluetoothNotFound)?;
        self.control_no_data(
            0x00,
            USB_REQUEST_SET_CONFIGURATION,
            u16::from(bluetooth.configuration),
            0,
        )?;
        self.configure_bluetooth_endpoints(bluetooth)?;
        self.bluetooth = Some(bluetooth);
        Ok(bluetooth)
    }

    fn configure_bluetooth_endpoints(
        &mut self,
        bluetooth: BluetoothUsbInterface,
    ) -> Result<(), InitError> {
        let event_dci = endpoint_dci(bluetooth.event_endpoint)?;
        let acl_in_dci = endpoint_dci(bluetooth.acl_in_endpoint)?;
        let acl_out_dci = endpoint_dci(bluetooth.acl_out_endpoint)?;
        if event_dci == acl_in_dci || event_dci == acl_out_dci || acl_in_dci == acl_out_dci {
            return Err(InitError::DescriptorInvalid);
        }

        let event_ring = Ring::new(DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?);
        let acl_in_ring = Ring::new(DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?);
        let acl_out_ring = Ring::new(DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?);
        let event_buffer = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let acl_in_buffer = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let acl_out_buffer = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let mut input = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        input.bytes_mut().fill(0);

        let output = self.output_context.ok_or(InitError::CommandFailed)?;
        input.bytes_mut()[self.context_size..self.context_size * 2]
            .copy_from_slice(&output.bytes()[..self.context_size]);
        let add_flags =
            1_u32 | (1_u32 << event_dci) | (1_u32 << acl_in_dci) | (1_u32 << acl_out_dci);
        write_u32(input.bytes_mut(), 4, add_flags);

        let slot_offset = self.context_size;
        let mut slot_dword0 = read_u32(input.bytes(), slot_offset);
        slot_dword0 &= !(0x1f << 27);
        let context_entries = event_dci.max(acl_in_dci).max(acl_out_dci);
        slot_dword0 |= u32::from(context_entries) << 27;
        write_u32(input.bytes_mut(), slot_offset, slot_dword0);

        write_bluetooth_endpoint_context(
            input.bytes_mut(),
            self.context_size,
            event_dci,
            bluetooth.event_endpoint,
            bluetooth.event_max_packet,
            bluetooth.event_interval,
            self.port_speed()?,
            event_ring.page.physical,
            USB_ENDPOINT_INTERRUPT,
        )?;
        write_bluetooth_endpoint_context(
            input.bytes_mut(),
            self.context_size,
            acl_in_dci,
            bluetooth.acl_in_endpoint,
            bluetooth.acl_in_max_packet,
            0,
            self.port_speed()?,
            acl_in_ring.page.physical,
            USB_ENDPOINT_BULK,
        )?;
        write_bluetooth_endpoint_context(
            input.bytes_mut(),
            self.context_size,
            acl_out_dci,
            bluetooth.acl_out_endpoint,
            bluetooth.acl_out_max_packet,
            0,
            self.port_speed()?,
            acl_out_ring.page.physical,
            USB_ENDPOINT_BULK,
        )?;
        fence(Ordering::Release);

        let command_ptr = self.command_ring.push(Trb {
            parameter: input.physical,
            status: 0,
            control: (TRB_TYPE_CONFIGURE_ENDPOINT << 10) | (u32::from(self.slot_id) << 24),
        });
        mmio_write32(self.doorbell_base, 0)?;
        self.wait_command_completion(command_ptr)?;

        self.bt_event_ring = Some(event_ring);
        self.bt_event_buffer = Some(event_buffer);
        self.bt_acl_in_ring = Some(acl_in_ring);
        self.bt_acl_in_buffer = Some(acl_in_buffer);
        self.bt_acl_out_ring = Some(acl_out_ring);
        self.bt_acl_out_buffer = Some(acl_out_buffer);
        Ok(())
    }

    fn send_bluetooth_command(&mut self, command: &[u8]) -> Result<(), InitError> {
        let bluetooth = self.bluetooth.ok_or(InitError::BluetoothNotFound)?;
        if command.is_empty() || command.len() > 258 {
            return Err(InitError::DescriptorInvalid);
        }
        self.control_out(
            USB_REQUEST_TYPE_BLUETOOTH_COMMAND,
            0,
            0,
            u16::from(bluetooth.interface),
            command,
        )
    }

    fn control_in(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buffer: &mut DmaPage,
        length: usize,
    ) -> Result<usize, InitError> {
        if length == 0 || length > PAGE_SIZE {
            return Err(InitError::DescriptorInvalid);
        }
        buffer.bytes_mut()[..length].fill(0);
        let setup = setup_packet(request_type, request, value, index, length as u16);
        let ring = self.ep0_ring.as_mut().ok_or(InitError::CommandFailed)?;
        ring.push(Trb {
            parameter: setup,
            status: 8,
            control: (TRB_TYPE_SETUP_STAGE << 10) | (1 << 6) | (3 << 16),
        });
        ring.push(Trb {
            parameter: buffer.physical,
            status: length as u32,
            control: (TRB_TYPE_DATA_STAGE << 10) | (1 << 16),
        });
        let status_ptr = ring.push(Trb {
            parameter: 0,
            status: 0,
            control: (TRB_TYPE_STATUS_STAGE << 10) | (1 << 5),
        });
        self.ring_endpoint(1)?;
        self.wait_transfer_completion(status_ptr, self.slot_id)?;
        fence(Ordering::Acquire);
        Ok(length)
    }

    fn control_out(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
    ) -> Result<(), InitError> {
        if data.is_empty() || data.len() > PAGE_SIZE {
            return Err(InitError::DescriptorInvalid);
        }
        let mut buffer = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        buffer.bytes_mut()[..data.len()].copy_from_slice(data);
        fence(Ordering::Release);
        let setup = setup_packet(request_type, request, value, index, data.len() as u16);
        let ring = self.ep0_ring.as_mut().ok_or(InitError::CommandFailed)?;
        ring.push(Trb {
            parameter: setup,
            status: 8,
            control: (TRB_TYPE_SETUP_STAGE << 10) | (1 << 6) | (2 << 16),
        });
        ring.push(Trb {
            parameter: buffer.physical,
            status: data.len() as u32,
            control: TRB_TYPE_DATA_STAGE << 10,
        });
        let status_ptr = ring.push(Trb {
            parameter: 0,
            status: 0,
            control: (TRB_TYPE_STATUS_STAGE << 10) | (1 << 16) | (1 << 5),
        });
        self.ring_endpoint(1)?;
        self.wait_transfer_completion(status_ptr, self.slot_id)?;
        Ok(())
    }

    fn control_no_data(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
    ) -> Result<(), InitError> {
        let setup = setup_packet(request_type, request, value, index, 0);
        let ring = self.ep0_ring.as_mut().ok_or(InitError::CommandFailed)?;
        ring.push(Trb {
            parameter: setup,
            status: 8,
            control: (TRB_TYPE_SETUP_STAGE << 10) | (1 << 6),
        });
        let status_ptr = ring.push(Trb {
            parameter: 0,
            status: 0,
            control: (TRB_TYPE_STATUS_STAGE << 10) | (1 << 16) | (1 << 5),
        });
        self.ring_endpoint(1)?;
        self.wait_transfer_completion(status_ptr, self.slot_id)?;
        Ok(())
    }

    fn configure_hid_endpoint(&mut self, hid: HidInterface) -> Result<(), InitError> {
        if hid.endpoint_address & 0x80 == 0 || hid.max_packet == 0 {
            return Err(InitError::DescriptorInvalid);
        }
        let endpoint_number = hid.endpoint_address & 0x0f;
        if endpoint_number == 0 {
            return Err(InitError::DescriptorInvalid);
        }
        let dci = endpoint_number.saturating_mul(2).saturating_add(1);
        if dci > 31 {
            return Err(InitError::DescriptorInvalid);
        }

        let hid_page = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let hid_ring = Ring::new(hid_page);
        let hid_buffer = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let mut input = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        input.bytes_mut().fill(0);

        let output = self.output_context.ok_or(InitError::CommandFailed)?;
        input.bytes_mut()[self.context_size..self.context_size * 2]
            .copy_from_slice(&output.bytes()[..self.context_size]);
        write_u32(input.bytes_mut(), 4, 1 | (1_u32 << dci));
        let slot_offset = self.context_size;
        let mut slot_dword0 = read_u32(input.bytes(), slot_offset);
        slot_dword0 &= !(0x1f << 27);
        slot_dword0 |= u32::from(dci) << 27;
        write_u32(input.bytes_mut(), slot_offset, slot_dword0);

        let endpoint_offset = self.context_size * (usize::from(dci) + 1);
        let interval = xhci_interval(self.port_speed()?, hid.interval);
        write_u32(
            input.bytes_mut(),
            endpoint_offset,
            u32::from(interval) << 16,
        );
        write_u32(
            input.bytes_mut(),
            endpoint_offset + 4,
            (3 << 1) | (7 << 3) | (u32::from(hid.max_packet) << 16),
        );
        write_u64(
            input.bytes_mut(),
            endpoint_offset + 8,
            hid_ring.page.physical | 1,
        );
        write_u32(
            input.bytes_mut(),
            endpoint_offset + 16,
            u32::from(hid.max_packet) | (u32::from(hid.max_packet) << 16),
        );
        fence(Ordering::Release);

        let command_ptr = self.command_ring.push(Trb {
            parameter: input.physical,
            status: 0,
            control: (TRB_TYPE_CONFIGURE_ENDPOINT << 10) | (u32::from(self.slot_id) << 24),
        });
        mmio_write32(self.doorbell_base, 0)?;
        self.wait_command_completion(command_ptr)?;
        self.hid_ring = Some(hid_ring);
        self.hid_buffer = Some(hid_buffer);
        Ok(())
    }

    pub fn poll_hid_report(&mut self) -> Result<[u8; 8], InitError> {
        let hid = self.hid.ok_or(InitError::HidNotFound)?;
        let endpoint_number = hid.endpoint_address & 0x0f;
        let dci = endpoint_number.saturating_mul(2).saturating_add(1);
        let transfer_length = cmp::min(usize::from(hid.max_packet), 8);
        if transfer_length == 0 {
            return Err(InitError::DescriptorInvalid);
        }
        let buffer_physical = {
            let buffer = self.hid_buffer.as_mut().ok_or(InitError::DmaUnavailable)?;
            buffer.bytes_mut()[..transfer_length].fill(0);
            buffer.physical
        };
        let normal_ptr = {
            let ring = self.hid_ring.as_mut().ok_or(InitError::CommandFailed)?;
            ring.push(Trb {
                parameter: buffer_physical,
                status: transfer_length as u32,
                control: (TRB_TYPE_NORMAL << 10) | (1 << 5),
            })
        };
        self.ring_endpoint(dci)?;
        self.wait_transfer_completion(normal_ptr, self.slot_id)?;
        fence(Ordering::Acquire);
        let buffer = self.hid_buffer.as_ref().ok_or(InitError::DmaUnavailable)?;
        let mut report = [0_u8; 8];
        report[..transfer_length].copy_from_slice(&buffer.bytes()[..transfer_length]);
        Ok(report)
    }

    fn receive_bluetooth_event(&mut self) -> Result<[u8; 257], InitError> {
        let bluetooth = self.bluetooth.ok_or(InitError::BluetoothNotFound)?;
        let dci = endpoint_dci(bluetooth.event_endpoint)?;
        let transfer_length = cmp::min(usize::from(bluetooth.event_max_packet), 257);
        if transfer_length == 0 {
            return Err(InitError::DescriptorInvalid);
        }
        let buffer_physical = {
            let buffer = self
                .bt_event_buffer
                .as_mut()
                .ok_or(InitError::DmaUnavailable)?;
            buffer.bytes_mut()[..transfer_length].fill(0);
            buffer.physical
        };
        let normal_ptr = {
            let ring = self
                .bt_event_ring
                .as_mut()
                .ok_or(InitError::CommandFailed)?;
            ring.push(Trb {
                parameter: buffer_physical,
                status: transfer_length as u32,
                control: (TRB_TYPE_NORMAL << 10) | (1 << 5),
            })
        };
        self.ring_endpoint(dci)?;
        self.wait_transfer_completion(normal_ptr, self.slot_id)?;
        fence(Ordering::Acquire);
        let buffer = self
            .bt_event_buffer
            .as_ref()
            .ok_or(InitError::DmaUnavailable)?;
        let mut event = [0_u8; 257];
        event[..transfer_length].copy_from_slice(&buffer.bytes()[..transfer_length]);
        Ok(event)
    }

    fn receive_bluetooth_acl(&mut self) -> Result<[u8; PAGE_SIZE], InitError> {
        let bluetooth = self.bluetooth.ok_or(InitError::BluetoothNotFound)?;
        let dci = endpoint_dci(bluetooth.acl_in_endpoint)?;
        let transfer_length = cmp::min(usize::from(bluetooth.acl_in_max_packet), PAGE_SIZE);
        if transfer_length == 0 {
            return Err(InitError::DescriptorInvalid);
        }
        let buffer_physical = {
            let buffer = self
                .bt_acl_in_buffer
                .as_mut()
                .ok_or(InitError::DmaUnavailable)?;
            buffer.bytes_mut()[..transfer_length].fill(0);
            buffer.physical
        };
        let normal_ptr = {
            let ring = self
                .bt_acl_in_ring
                .as_mut()
                .ok_or(InitError::CommandFailed)?;
            ring.push(Trb {
                parameter: buffer_physical,
                status: transfer_length as u32,
                control: (TRB_TYPE_NORMAL << 10) | (1 << 5),
            })
        };
        self.ring_endpoint(dci)?;
        self.wait_transfer_completion(normal_ptr, self.slot_id)?;
        fence(Ordering::Acquire);
        let buffer = self
            .bt_acl_in_buffer
            .as_ref()
            .ok_or(InitError::DmaUnavailable)?;
        let mut data = [0_u8; PAGE_SIZE];
        data[..transfer_length].copy_from_slice(&buffer.bytes()[..transfer_length]);
        Ok(data)
    }

    fn send_bluetooth_acl(&mut self, data: &[u8]) -> Result<(), InitError> {
        let bluetooth = self.bluetooth.ok_or(InitError::BluetoothNotFound)?;
        if data.is_empty() || data.len() > usize::from(bluetooth.acl_out_max_packet) {
            return Err(InitError::DescriptorInvalid);
        }
        let dci = endpoint_dci(bluetooth.acl_out_endpoint)?;
        let buffer_physical = {
            let buffer = self
                .bt_acl_out_buffer
                .as_mut()
                .ok_or(InitError::DmaUnavailable)?;
            buffer.bytes_mut()[..data.len()].copy_from_slice(data);
            buffer.physical
        };
        fence(Ordering::Release);
        let normal_ptr = {
            let ring = self
                .bt_acl_out_ring
                .as_mut()
                .ok_or(InitError::CommandFailed)?;
            ring.push(Trb {
                parameter: buffer_physical,
                status: data.len() as u32,
                control: (TRB_TYPE_NORMAL << 10) | (1 << 5),
            })
        };
        self.ring_endpoint(dci)?;
        self.wait_transfer_completion(normal_ptr, self.slot_id)?;
        Ok(())
    }

    fn ring_endpoint(&self, dci: u8) -> Result<(), InitError> {
        mmio_write32(
            self.doorbell_base + (u64::from(self.slot_id) * 4),
            u32::from(dci),
        )
    }

    fn wait_command_completion(&mut self, command_ptr: u64) -> Result<Trb, InitError> {
        for _ in 0..MAX_EVENTS_PER_WAIT {
            let event = self.wait_event()?;
            if trb_type(event.control) != TRB_TYPE_COMMAND_COMPLETION {
                continue;
            }
            if event.parameter != command_ptr || completion_code(event.status) != 1 {
                return Err(InitError::CommandFailed);
            }
            return Ok(event);
        }
        Err(InitError::CommandFailed)
    }

    fn wait_transfer_completion(&mut self, trb_ptr: u64, slot: u8) -> Result<Trb, InitError> {
        for _ in 0..MAX_EVENTS_PER_WAIT {
            let event = self.wait_event()?;
            if trb_type(event.control) != TRB_TYPE_TRANSFER_EVENT {
                continue;
            }
            if ((event.control >> 24) & 0xff) as u8 != slot || event.parameter != trb_ptr {
                continue;
            }
            let code = completion_code(event.status);
            if code == 1 || code == 13 {
                return Ok(event);
            }
            return Err(InitError::TransferFailed);
        }
        Err(InitError::TransferFailed)
    }

    fn wait_event(&mut self) -> Result<Trb, InitError> {
        for _ in 0..POLL_LIMIT {
            if let Some(event) = self.try_event()? {
                return Ok(event);
            }
            core::hint::spin_loop();
        }
        Err(InitError::ControllerTimeout)
    }

    /// The next event, if the controller has written one; never waits.
    fn try_event(&mut self) -> Result<Option<Trb>, InitError> {
        {
            fence(Ordering::Acquire);
            let offset = self.event_index * 16;
            let bytes = self.event_ring.bytes_mut();
            let control = u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap());
            if (control & 1 != 0) == self.event_cycle {
                let event = Trb {
                    parameter: u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()),
                    status: u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()),
                    control,
                };
                self.event_index += 1;
                if self.event_index == PAGE_SIZE / 16 {
                    self.event_index = 0;
                    self.event_cycle = !self.event_cycle;
                }
                let dequeue = self.event_ring.physical + (self.event_index as u64 * 16);
                mmio_write64(self.runtime_base + 0x20 + 0x18, dequeue | (1 << 3))?;
                return Ok(Some(event));
            }
        }
        Ok(None)
    }

    /// Stage 13.13: enumerate a keyboard or mouse on every connected root
    /// port, each with its own slot, and arm one interrupt transfer for each.
    /// A port whose device is not a boot keyboard or mouse is skipped.
    #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
    fn enumerate_inputs(&mut self) -> usize {
        let mut count = self.inputs.iter().flatten().count();
        for port in 1..=self.max_ports {
            if count == MAX_INPUTS {
                break;
            }
            if self
                .inputs
                .iter()
                .flatten()
                .any(|device| device.port == port)
            {
                continue;
            }
            let portsc = self.op_base + 0x400 + (u64::from(port - 1) * 0x10);
            if mmio_read32(portsc).map_or(true, |value| value & 1 == 0) {
                continue;
            }
            // The first connected port was reset and given a slot when the
            // controller started; every other port gets its own here.
            if port != self.connected_port || self.slot_id == 0 {
                if self.reset_port(port).is_err() {
                    continue;
                }
                self.connected_port = port;
                self.output_context = None;
                self.ep0_ring = None;
                self.hid_ring = None;
                self.hid_buffer = None;
                self.hid = None;
                self.slot_id = match self.enable_slot() {
                    Ok(slot) => slot,
                    Err(_) => continue,
                };
            }
            // A mass-storage device becomes the USB disk (Stage 13.18);
            // anything else is tried as an input device.
            let Ok((descriptor, total_length, configuration)) = self
                .address_device()
                .and_then(|()| self.read_configuration())
            else {
                self.slot_id = 0;
                continue;
            };
            if self.storage.is_none() {
                if let Some(interface) = crate::usb_msc::parse_interface(
                    &descriptor.bytes()[..total_length],
                    configuration,
                ) {
                    match self.attach_storage(port, interface) {
                        Ok(device) => {
                            crate::serial::write_line(format_args!(
                                "[USB] mass storage on port {}: {} blocks of {} bytes",
                                port, device.blocks, device.block_size
                            ));
                            self.storage = Some(device);
                        }
                        Err(error) => {
                            crate::serial::write_line(format_args!(
                                "[USB] mass storage on port {}: {:?}",
                                port, error
                            ));
                        }
                    }
                    self.slot_id = 0;
                    self.hid = None;
                    continue;
                }
            }
            let summary = self.finish_hid(&descriptor, total_length, configuration);
            let device = summary.ok().and_then(|summary| {
                let hid = self.hid?;
                // Boot keyboards and mice have fixed reports; anything else
                // must describe its reports in a layout this kernel decodes.
                let layout = if summary.kind == HidKind::Other {
                    Some(self.report_layout(hid)?)
                } else {
                    None
                };
                // Report only on change: the enumeration's short idle period
                // exists for the Stage 13.6 probe, not for input devices.
                self.control_no_data(0x21, USB_REQUEST_SET_IDLE, 0, u16::from(hid.interface))
                    .ok()?;
                let limit = if layout.is_some() { MAX_REPORT } else { 8 };
                Some(InputDevice {
                    slot_id: self.slot_id,
                    port,
                    kind: summary.kind,
                    layout,
                    dci: (hid.endpoint_address & 0x0f)
                        .saturating_mul(2)
                        .saturating_add(1),
                    length: cmp::min(usize::from(hid.max_packet), limit),
                    ring: self.hid_ring.take()?,
                    buffer: self.hid_buffer.take()?,
                    pending: 0,
                    _output_context: self.output_context.take(),
                    _ep0_ring: self.ep0_ring.take(),
                })
            });
            // The active slot is consumed either way; the next port starts
            // from a fresh one.
            self.slot_id = 0;
            self.hid = None;
            let Some(mut device) = device else { continue };
            if arm_input(self.doorbell_base, &mut device).is_err() {
                continue;
            }
            if let Some(slot) = self.inputs.iter_mut().find(|slot| slot.is_none()) {
                *slot = Some(device);
                count += 1;
            }
        }
        count
    }

    /// Fetch and parse the active device's report descriptor.
    #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
    fn report_layout(&mut self, hid: HidInterface) -> Option<crate::hid::Layout> {
        let length = usize::from(hid.report_length);
        if length == 0 || length > PAGE_SIZE {
            return None;
        }
        let mut descriptor = DmaPage::allocate_zeroed()?;
        // GET_DESCRIPTOR(Report), addressed to the interface.
        self.control_in(
            0x81,
            USB_REQUEST_GET_DESCRIPTOR,
            0x22 << 8,
            u16::from(hid.interface),
            &mut descriptor,
            length,
        )
        .ok()?;
        crate::hid::parse(&descriptor.bytes()[..length]).ok()
    }

    /// Let the controller interrupt: interrupter 0 enabled, moderated to at
    /// most one interrupt per millisecond, and USBCMD.INTE set.
    #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
    fn enable_interrupts(&mut self) -> Result<(), InitError> {
        let interrupter = self.runtime_base + 0x20;
        mmio_write32(interrupter + 0x04, 4000)?;
        // IMAN: IP is write-one-to-clear; IE enables the interrupter.
        mmio_write32(interrupter, 0b11)?;
        let command = mmio_read32(self.op_base)?;
        mmio_write32(self.op_base, command | (1 << 2))
    }

    /// Drain the event ring: copy each completed input report out and re-arm
    /// its transfer. Runs under the controller lock and never waits; the
    /// reports are decoded and published by the caller after unlocking.
    #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
    fn service_inputs(
        &mut self,
        reports: &mut [(HidKind, usize, [u8; MAX_REPORT]); REPORTS_PER_PASS],
    ) -> usize {
        // Acknowledge first, so an event written while draining raises a
        // fresh interrupt rather than being folded into this one.
        let _ = mmio_write32(self.runtime_base + 0x20, 0b11);
        let mut count = 0;
        // Reports a storage transfer drained come first, in arrival order.
        let stashed = self.stashed_len.min(REPORTS_PER_PASS);
        reports[..stashed].copy_from_slice(&self.stashed[..stashed]);
        self.stashed.copy_within(stashed..self.stashed_len, 0);
        self.stashed_len -= stashed;
        count += stashed;
        while count < REPORTS_PER_PASS {
            let Ok(Some(event)) = self.try_event() else {
                break;
            };
            if let Some(report) = self.route_input_event(event) {
                reports[count] = report;
                count += 1;
            }
        }
        count
    }

    /// An input device's completed transfer: copy its report out and re-arm
    /// it. `None` for any other event, or a completion without a report.
    #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
    fn route_input_event(&mut self, event: Trb) -> Option<(HidKind, usize, [u8; MAX_REPORT])> {
        if trb_type(event.control) != TRB_TYPE_TRANSFER_EVENT {
            return None;
        }
        let doorbell_base = self.doorbell_base;
        let slot = ((event.control >> 24) & 0xff) as u8;
        let (index, device) = self
            .inputs
            .iter_mut()
            .enumerate()
            .find_map(|(index, device)| {
                device
                    .as_mut()
                    .filter(|device| device.slot_id == slot && device.pending == event.parameter)
                    .map(|device| (index, device))
            })?;
        let code = completion_code(event.status);
        let report = (code == 1 || code == 13).then(|| {
            fence(Ordering::Acquire);
            let mut report = [0u8; MAX_REPORT];
            report[..device.length].copy_from_slice(&device.buffer.bytes()[..device.length]);
            INPUT_REPORTS.fetch_add(1, Ordering::Relaxed);
            (device.kind, index, report)
        });
        // Keep the device's stream alive whatever the completion said.
        let _ = arm_input(doorbell_base, device);
        report
    }

    pub fn summary(&self) -> (u8, u8, u8, u8) {
        (
            self.max_slots,
            self.max_ports,
            self.connected_port,
            self.slot_id,
        )
    }
}

/// Stage 13.18: the USB disk.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
impl XhciController {
    /// Configure the addressed device's Bulk-Only interface and bring the
    /// disk up: INQUIRY, TEST UNIT READY until ready, READ CAPACITY. Only
    /// 512-byte blocks addressable by 10-byte commands are accepted.
    fn attach_storage(
        &mut self,
        port: u8,
        interface: crate::usb_msc::Interface,
    ) -> Result<StorageDevice, InitError> {
        let in_dci = endpoint_dci(interface.bulk_in)?;
        let out_dci = endpoint_dci(interface.bulk_out)?;
        if in_dci == out_dci {
            return Err(InitError::DescriptorInvalid);
        }
        self.control_no_data(
            0x00,
            USB_REQUEST_SET_CONFIGURATION,
            u16::from(interface.configuration),
            0,
        )?;

        let in_ring = Ring::new(DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?);
        let out_ring = Ring::new(DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?);
        let mut input = DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?;
        let output = self
            .output_context
            .as_ref()
            .ok_or(InitError::CommandFailed)?;
        input.bytes_mut()[self.context_size..self.context_size * 2]
            .copy_from_slice(&output.bytes()[..self.context_size]);
        write_u32(
            input.bytes_mut(),
            4,
            1 | (1_u32 << in_dci) | (1_u32 << out_dci),
        );
        let slot_offset = self.context_size;
        let slot_dword0 = (read_u32(input.bytes(), slot_offset) & !(0x1f << 27))
            | (u32::from(in_dci.max(out_dci)) << 27);
        write_u32(input.bytes_mut(), slot_offset, slot_dword0);
        let speed = self.port_speed()?;
        write_bluetooth_endpoint_context(
            input.bytes_mut(),
            self.context_size,
            in_dci,
            interface.bulk_in,
            interface.bulk_in_max_packet,
            0,
            speed,
            in_ring.page.physical,
            USB_ENDPOINT_BULK,
        )?;
        write_bluetooth_endpoint_context(
            input.bytes_mut(),
            self.context_size,
            out_dci,
            interface.bulk_out,
            interface.bulk_out_max_packet,
            0,
            speed,
            out_ring.page.physical,
            USB_ENDPOINT_BULK,
        )?;
        fence(Ordering::Release);
        let command_ptr = self.command_ring.push(Trb {
            parameter: input.physical,
            status: 0,
            control: (TRB_TYPE_CONFIGURE_ENDPOINT << 10) | (u32::from(self.slot_id) << 24),
        });
        mmio_write32(self.doorbell_base, 0)?;
        self.wait_command_completion(command_ptr)?;

        let mut device = StorageDevice {
            slot_id: self.slot_id,
            port,
            interface,
            in_dci,
            out_dci,
            in_ring,
            out_ring,
            data: DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?,
            wrappers: DmaPage::allocate_zeroed().ok_or(InitError::DmaUnavailable)?,
            tag: 1,
            blocks: 0,
            block_size: 0,
            vendor: [0; 8],
            product: [0; 16],
            output_context: self.output_context.take(),
            ep0_ring: self.ep0_ring.take(),
        };

        let inquiry = self.scsi(&mut device, &crate::usb_msc::inquiry(36), Direction::In(36));
        if inquiry.is_ok() {
            device.vendor.copy_from_slice(&device.data.bytes()[8..16]);
            device.product.copy_from_slice(&device.data.bytes()[16..32]);
        }
        // A disk reports UNIT ATTENTION, or not ready, until it settles;
        // REQUEST SENSE clears the condition between attempts.
        let mut ready = false;
        for _ in 0..READY_ATTEMPTS {
            if self
                .scsi(
                    &mut device,
                    &crate::usb_msc::test_unit_ready(),
                    Direction::None,
                )
                .is_ok()
            {
                ready = true;
                break;
            }
            let sensed = self
                .scsi(
                    &mut device,
                    &crate::usb_msc::request_sense(18),
                    Direction::In(18),
                )
                .is_ok();
            // NOT READY, MEDIUM NOT PRESENT: an empty card reader never settles.
            if sensed
                && matches!(
                    crate::usb_msc::parse_sense(&device.data.bytes()[..18]),
                    Some((0x02, 0x3a, _))
                )
            {
                return Err(InitError::TransferFailed);
            }
        }
        if !ready {
            return Err(InitError::TransferFailed);
        }
        self.scsi(
            &mut device,
            &crate::usb_msc::read_capacity(),
            Direction::In(8),
        )
        .map_err(|_| InitError::TransferFailed)?;
        let (blocks, block_size) = crate::usb_msc::parse_capacity(&device.data.bytes()[..8])
            .ok_or(InitError::DescriptorInvalid)?;
        if block_size as usize != crate::block::SECTOR_SIZE || blocks > u64::from(u32::MAX) + 1 {
            return Err(InitError::DescriptorInvalid);
        }
        device.blocks = blocks;
        device.block_size = block_size;
        Ok(device)
    }

    /// One Bulk-Only command: the command wrapper, the data stage into or
    /// out of `device.data`, and the status wrapper. Returns the bytes the
    /// data stage moved.
    fn scsi(
        &mut self,
        device: &mut StorageDevice,
        command: &[u8],
        direction: Direction,
    ) -> Result<usize, StorageError> {
        let (length, data_in) = match direction {
            Direction::None => (0, false),
            Direction::In(length) => (length, true),
            Direction::Out(length) => (length, false),
        };
        if length > PAGE_SIZE {
            return Err(StorageError::NoDisk);
        }
        let tag = device.tag;
        device.tag = device.tag.wrapping_add(1).max(1);
        let block = crate::usb_msc::command_block(tag, length as u32, data_in, command)
            .ok_or(StorageError::NoDisk)?;
        device.wrappers.bytes_mut()[..crate::usb_msc::CBW_LENGTH].copy_from_slice(&block);
        let wrappers = device.wrappers.physical;
        self.bulk(device, false, wrappers, crate::usb_msc::CBW_LENGTH)?;

        let mut moved = 0;
        if length > 0 {
            let data = device.data.physical;
            match self.bulk(device, data_in, data, length) {
                Ok(count) => moved = count,
                // A stalled data stage still ends with a status wrapper,
                // once the endpoint's halt is cleared.
                Err(BulkError::Stall) => self.clear_halt(device, data_in)?,
                Err(error) => return Err(error.into()),
            }
        }

        const STATUS: usize = 256;
        device.wrappers.bytes_mut()[STATUS..STATUS + crate::usb_msc::CSW_LENGTH].fill(0);
        let status = match self.bulk(
            device,
            true,
            wrappers + STATUS as u64,
            crate::usb_msc::CSW_LENGTH,
        ) {
            Err(BulkError::Stall) => {
                // A stalled status stage is retried once after clearing it.
                self.clear_halt(device, true)?;
                self.bulk(
                    device,
                    true,
                    wrappers + STATUS as u64,
                    crate::usb_msc::CSW_LENGTH,
                )?
            }
            other => other?,
        };
        if status != crate::usb_msc::CSW_LENGTH {
            return Err(StorageError::Transport);
        }
        fence(Ordering::Acquire);
        let wrapper = &device.wrappers.bytes()[STATUS..STATUS + crate::usb_msc::CSW_LENGTH];
        match crate::usb_msc::parse_status(wrapper, tag, length as u32) {
            Ok(crate::usb_msc::Status::Passed { residue }) => {
                Ok(moved.min(length - residue as usize))
            }
            Ok(crate::usb_msc::Status::Failed) => Err(StorageError::CheckCondition),
            Ok(crate::usb_msc::Status::PhaseError) | Err(_) => Err(StorageError::Transport),
        }
    }

    /// One bulk transfer of `length` bytes at `physical`; returns the bytes
    /// moved. Events for other devices seen while waiting are routed: input
    /// reports are stashed for the input worker.
    fn bulk(
        &mut self,
        device: &mut StorageDevice,
        data_in: bool,
        physical: u64,
        length: usize,
    ) -> Result<usize, BulkError> {
        let (ring, dci) = if data_in {
            (&mut device.in_ring, device.in_dci)
        } else {
            (&mut device.out_ring, device.out_dci)
        };
        // Normal TRB, interrupt on completion, and for IN, an event on a
        // short packet so a short read completes.
        let short = if data_in { 1 << 2 } else { 0 };
        let pending = ring.push(Trb {
            parameter: physical,
            status: length as u32,
            control: (TRB_TYPE_NORMAL << 10) | (1 << 5) | short,
        });
        fence(Ordering::Release);
        mmio_write32(
            self.doorbell_base + u64::from(device.slot_id) * 4,
            u32::from(dci),
        )
        .map_err(|_| BulkError::Fault)?;
        for _ in 0..POLL_LIMIT {
            let Some(event) = self.try_event().map_err(|_| BulkError::Fault)? else {
                core::hint::spin_loop();
                continue;
            };
            let slot = ((event.control >> 24) & 0xff) as u8;
            if trb_type(event.control) == TRB_TYPE_TRANSFER_EVENT
                && slot == device.slot_id
                && event.parameter == pending
            {
                let residual = (event.status & 0x00ff_ffff) as usize;
                return match completion_code(event.status) {
                    1 | 13 => Ok(length.saturating_sub(residual)),
                    COMPLETION_STALL => Err(BulkError::Stall),
                    _ => Err(BulkError::Fault),
                };
            }
            if let Some(report) = self.route_input_event(event) {
                if self.stashed_len < STASHED_REPORTS {
                    self.stashed[self.stashed_len] = report;
                    self.stashed_len += 1;
                }
            }
        }
        Err(BulkError::Timeout)
    }

    /// Clear a halted bulk endpoint: Reset Endpoint, point its ring past
    /// the stalled transfer, and CLEAR_FEATURE(ENDPOINT_HALT) on the device.
    fn clear_halt(
        &mut self,
        device: &mut StorageDevice,
        data_in: bool,
    ) -> Result<(), StorageError> {
        let (dci, address) = if data_in {
            (device.in_dci, device.interface.bulk_in)
        } else {
            (device.out_dci, device.interface.bulk_out)
        };
        let slot = u32::from(device.slot_id) << 24;
        let endpoint = u32::from(dci) << 16;
        let command = self.command_ring.push(Trb {
            parameter: 0,
            status: 0,
            control: (TRB_TYPE_RESET_ENDPOINT << 10) | slot | endpoint,
        });
        mmio_write32(self.doorbell_base, 0).map_err(|_| StorageError::Transport)?;
        self.wait_command_completion(command)
            .map_err(|_| StorageError::Transport)?;
        let ring = if data_in {
            &device.in_ring
        } else {
            &device.out_ring
        };
        let dequeue = (ring.page.physical + ring.index as u64 * 16) | u64::from(ring.cycle);
        let command = self.command_ring.push(Trb {
            parameter: dequeue,
            status: 0,
            control: (TRB_TYPE_SET_TR_DEQUEUE << 10) | slot | endpoint,
        });
        mmio_write32(self.doorbell_base, 0).map_err(|_| StorageError::Transport)?;
        self.wait_command_completion(command)
            .map_err(|_| StorageError::Transport)?;
        // The control pipe is the disk's: lend it to the control helpers.
        let (saved_slot, saved_ring) = (self.slot_id, self.ep0_ring.take());
        self.slot_id = device.slot_id;
        self.ep0_ring = device.ep0_ring.take();
        let cleared = self.control_no_data(0x02, 1, 0, u16::from(address));
        device.ep0_ring = self.ep0_ring.take();
        self.slot_id = saved_slot;
        self.ep0_ring = saved_ring;
        cleared.map_err(|_| StorageError::Transport)
    }

    /// Read or write one 512-byte block of the USB disk.
    fn storage_block(
        &mut self,
        lba: u64,
        write: Option<&[u8]>,
        read: Option<&mut [u8]>,
    ) -> Result<(), StorageError> {
        let mut device = self.storage.take().ok_or(StorageError::NoDisk)?;
        let result = (|| {
            let block = u32::try_from(lba)
                .ok()
                .filter(|_| lba < device.blocks)
                .ok_or(StorageError::NoDisk)?;
            let size = crate::block::SECTOR_SIZE;
            let moved = if let Some(data) = write {
                device.data.bytes_mut()[..size].copy_from_slice(data);
                fence(Ordering::Release);
                self.scsi(
                    &mut device,
                    &crate::usb_msc::read_write(true, block, 1),
                    Direction::Out(size),
                )?
            } else {
                self.scsi(
                    &mut device,
                    &crate::usb_msc::read_write(false, block, 1),
                    Direction::In(size),
                )?
            };
            if moved != size {
                return Err(StorageError::Transport);
            }
            if let Some(out) = read {
                out.copy_from_slice(&device.data.bytes()[..size]);
            }
            Ok(())
        })();
        self.storage = Some(device);
        result
    }

    fn storage_flush(&mut self) -> Result<(), StorageError> {
        let mut device = self.storage.take().ok_or(StorageError::NoDisk)?;
        let result = self
            .scsi(
                &mut device,
                &crate::usb_msc::synchronize_cache(),
                Direction::None,
            )
            .map(|_| ());
        self.storage = Some(device);
        result
    }
}

/// The data stage of a Bulk-Only command.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
#[derive(Clone, Copy)]
enum Direction {
    None,
    In(usize),
    Out(usize),
}

/// Why one bulk transfer failed.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BulkError {
    Stall,
    Fault,
    Timeout,
}

#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
impl From<BulkError> for StorageError {
    fn from(error: BulkError) -> Self {
        match error {
            BulkError::Timeout => StorageError::Timeout,
            BulkError::Stall | BulkError::Fault => StorageError::Transport,
        }
    }
}

/// The USB disk as a block device. Each block takes the controller lock
/// for one Bulk-Only command and releases it, so a long run of I/O never
/// holds the lock, or keeps interrupts masked, for more than one block.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
pub struct UsbDisk;

#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
impl crate::block::BlockDevice for UsbDisk {
    fn sector_count(&self) -> u64 {
        CONTROLLER
            .lock()
            .as_ref()
            .and_then(|controller| controller.storage.as_ref())
            .map_or(0, |device| device.blocks)
    }

    fn flush(&mut self) -> Result<(), crate::block::Error> {
        let mut slot = CONTROLLER.lock();
        let controller = slot.as_mut().ok_or(crate::block::Error::DeviceFault)?;
        controller.storage_flush().map_err(storage_error)
    }

    fn read_sector(&mut self, lba: u64, sector: &mut [u8]) -> Result<(), crate::block::Error> {
        if sector.len() != crate::block::SECTOR_SIZE {
            return Err(crate::block::Error::InvalidBuffer);
        }
        let mut slot = CONTROLLER.lock();
        let controller = slot.as_mut().ok_or(crate::block::Error::DeviceFault)?;
        controller
            .storage_block(lba, None, Some(sector))
            .map_err(storage_error)
    }

    fn write_sector(&mut self, lba: u64, sector: &[u8]) -> Result<(), crate::block::Error> {
        if sector.len() != crate::block::SECTOR_SIZE {
            return Err(crate::block::Error::InvalidBuffer);
        }
        let mut slot = CONTROLLER.lock();
        let controller = slot.as_mut().ok_or(crate::block::Error::DeviceFault)?;
        controller
            .storage_block(lba, Some(sector), None)
            .map_err(storage_error)
    }
}

#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
fn storage_error(error: StorageError) -> crate::block::Error {
    match error {
        StorageError::NoDisk => crate::block::Error::OutOfBounds,
        _ => crate::block::Error::DeviceFault,
    }
}

/// What the USB disk reported when it was attached.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbDiskInfo {
    pub port: u8,
    pub blocks: u64,
    pub vendor: [u8; 8],
    pub product: [u8; 16],
}

/// The attached USB disk, if any (enumerated by `start_input`).
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
pub fn usb_disk_info() -> Option<UsbDiskInfo> {
    let slot = CONTROLLER.lock();
    let device = slot.as_ref()?.storage.as_ref()?;
    Some(UsbDiskInfo {
        port: device.port,
        blocks: device.blocks,
        vendor: device.vendor,
        product: device.product,
    })
}

/// Whether the controller lock is free right now (for a panic handler,
/// which must not wait on a lock the panicking code may hold).
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
pub fn controller_free() -> bool {
    CONTROLLER.try_lock().is_some()
}

/// Run `operation` on the USB disk, if one is attached.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
pub fn with_usb_disk<R>(operation: impl FnOnce(&mut UsbDisk) -> R) -> Option<R> {
    usb_disk_info()?;
    Some(operation(&mut UsbDisk))
}

/// Queue one interrupt-IN transfer for `device` and ring its doorbell.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
fn arm_input(doorbell_base: u64, device: &mut InputDevice) -> Result<(), InitError> {
    device.buffer.bytes_mut()[..MAX_REPORT].fill(0);
    device.pending = device.ring.push(Trb {
        parameter: device.buffer.physical,
        status: device.length as u32,
        // Normal TRB, interrupt on completion.
        control: (TRB_TYPE_NORMAL << 10) | (1 << 5),
    });
    fence(Ordering::Release);
    mmio_write32(
        doorbell_base + (u64::from(device.slot_id) * 4),
        u32::from(device.dci),
    )
}

/// The input interrupt vector (0 until input delivery starts).
static INPUT_VECTOR: AtomicU8 = AtomicU8::new(0);
/// Cooperative stop requested for the HID worker before controller teardown.
static INPUT_STOP: AtomicU8 = AtomicU8::new(0);
/// The worker task must be dead before its waiter, vector, and DMA rings are
/// released. A raw ID avoids taking a scheduler lock from interrupt paths.
static INPUT_TASK: AtomicU64 = AtomicU64::new(0);
static DEFERRED_STOP: AtomicU8 = AtomicU8::new(0);
static DEFERRED_TASK: AtomicU64 = AtomicU64::new(0);
/// Reports copied out of the controller, and worker passes, for the gate.
static INPUT_REPORTS: AtomicU64 = AtomicU64::new(0);
static INPUT_PASSES: AtomicU64 = AtomicU64::new(0);

/// What input delivery found and the vector it runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputSummary {
    pub keyboards: u8,
    pub mice: u8,
    pub vector: u8,
}

/// Stage 13.13: deliver the controller's keyboards and mice into WovenInput.
/// Enumerates every connected port, binds an MSI/MSI-X vector through the
/// driver manager for `driver`, and starts a worker that sleeps until that
/// vector fires. Without any keyboard or mouse, nothing is started.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
pub fn start_input(driver: &'static str) -> Result<InputSummary, InitError> {
    let (keyboards, mice) = {
        let mut slot = CONTROLLER.lock();
        let controller = slot.as_mut().ok_or(InitError::MissingController)?;
        controller.enumerate_inputs();
        count_inputs(controller)
    };
    if keyboards + mice == 0 {
        return Ok(InputSummary {
            keyboards,
            mice,
            vector: 0,
        });
    }
    if let Some(vector) =
        (INPUT_VECTOR.load(Ordering::Acquire) != 0).then(|| INPUT_VECTOR.load(Ordering::Acquire))
    {
        // Enumeration above still runs when the HID interrupt worker already
        // exists: a storage device may settle after a keyboard or mouse. Do
        // not reinstall the vector or spawn a second input worker.
        return Ok(InputSummary {
            keyboards,
            mice,
            vector,
        });
    }
    let vector = match crate::driver::enable_pci_interrupt(driver, crate::smp::bsp_apic_id(), 0) {
        Ok(crate::driver::PciInterruptLease::Msi(lease)) => lease.vector.vector,
        Ok(crate::driver::PciInterruptLease::Msix(lease)) => lease.vector.vector,
        Err(_) => return Err(InitError::InterruptUnavailable),
    };
    INPUT_VECTOR.store(vector, Ordering::Release);
    let worker = crate::task::spawn_io_service("usb-input", input_worker)
        .map_err(|_| InitError::InterruptUnavailable)?;
    INPUT_STOP.store(0, Ordering::Release);
    INPUT_TASK.store(worker.as_u64(), Ordering::Release);
    if !crate::interrupts::set_pci_vector_waiter(vector, worker) {
        INPUT_TASK.store(0, Ordering::Release);
        INPUT_STOP.store(1, Ordering::Release);
        let _ = crate::task::signal_event(worker);
        return Err(InitError::InterruptUnavailable);
    }
    CONTROLLER
        .lock()
        .as_mut()
        .ok_or(InitError::MissingController)?
        .enable_interrupts()?;
    // Reports that completed before interrupts were enabled raised none.
    let _ = crate::task::signal_event(worker);
    Ok(InputSummary {
        keyboards,
        mice,
        vector,
    })
}

/// Start physical USB input and mass-storage enumeration outside boot-critical
/// driver bring-up. A real device may take several bounded transfer polls to
/// settle; keeping that work on the service queue lets the session and the
/// deferred live logger start while enumeration proceeds.
#[cfg(not(feature = "qemu-test"))]
pub fn start_input_deferred(driver: &'static str) -> Result<(), crate::task::SpawnError> {
    let _ = driver;
    DEFERRED_STOP.store(0, Ordering::Release);
    let task = crate::task::spawn_io_service("usb-enumerate", deferred_input_worker)?;
    DEFERRED_TASK.store(task.as_u64(), Ordering::Release);
    Ok(())
}

#[cfg(not(feature = "qemu-test"))]
fn deferred_input_worker() -> ! {
    const RETRY_TICKS: u64 = 300;
    let mut attempt = 0u32;
    loop {
        if DEFERRED_STOP.load(Ordering::Acquire) != 0 {
            DEFERRED_TASK.store(0, Ordering::Release);
            crate::task::exit_current_task();
        }
        attempt = attempt.saturating_add(1);
        let result = start_input("xhci0");
        if usb_disk_info().is_some() {
            match result {
                Ok(summary) => crate::serial::write_line(format_args!(
                    "[USB] deferred enumeration complete: keyboards={} mice={} vector={:#x} attempt={}",
                    summary.keyboards, summary.mice, summary.vector, attempt
                )),
                Err(error) => crate::serial::write_line(format_args!(
                    "[USB] mass storage ready; input enumeration incomplete: {:?} attempt={}",
                    error, attempt
                )),
            }
            break;
        }
        match result {
            Ok(summary) => crate::serial::write_line(format_args!(
                "[USB] deferred enumeration found no storage: keyboards={} mice={} vector={:#x} attempt={}; retrying",
                summary.keyboards, summary.mice, summary.vector, attempt
            )),
            Err(error) => crate::serial::write_line(format_args!(
                "[USB] deferred enumeration failed: {:?} attempt={}; retrying",
                error, attempt
            )),
        }
        crate::task::sleep_current(RETRY_TICKS);
    }
    crate::task::exit_current_task()
}

#[cfg(feature = "stage31-driver-restart-test")]
fn stop_task(raw: &core::sync::atomic::AtomicU64, stop: &AtomicU8) -> Result<(), InitError> {
    let id = raw.load(Ordering::Acquire);
    if id == 0 {
        return Ok(());
    }
    stop.store(1, Ordering::Release);
    let task = crate::task::TaskId::from_u64(id);
    let _ = crate::task::signal_event(task);
    crate::serial::write_line(format_args!("[XHCI] shutdown: stop requested id={:#x}", id));
    for _ in 0..POLL_LIMIT {
        if !crate::task::task_live(task) {
            return Ok(());
        }
        crate::task::yield_now();
    }
    Err(InitError::ControllerTimeout)
}

/// Stop all xHCI-owned workers and remove the interrupt waiter only after the
/// worker has exited. This closes the race between task teardown and DMA-ring
/// release.
#[cfg(feature = "stage31-driver-restart-test")]
pub fn shutdown() -> Result<(), InitError> {
    crate::serial::write_line(format_args!("[XHCI] shutdown: stopping deferred worker"));
    stop_task(&DEFERRED_TASK, &DEFERRED_STOP)?;
    crate::serial::write_line(format_args!("[XHCI] shutdown: deferred worker stopped"));
    let input_task_id = INPUT_TASK.load(Ordering::Acquire);
    crate::serial::write_line(format_args!(
        "[XHCI] shutdown: stopping input worker id={:#x}",
        input_task_id
    ));
    stop_task(&INPUT_TASK, &INPUT_STOP)?;
    crate::serial::write_line(format_args!("[XHCI] shutdown: input worker stopped"));
    let vector = INPUT_VECTOR.load(Ordering::Acquire);
    if vector != 0 {
        crate::serial::write_line(format_args!(
            "[XHCI] shutdown: clearing waiter vector={:#x}",
            vector
        ));
        let task = crate::task::TaskId::from_u64(input_task_id);
        let _ = crate::interrupts::clear_pci_vector_waiter(vector, task);
        INPUT_VECTOR.store(0, Ordering::Release);
    }
    INPUT_TASK.store(0, Ordering::Release);
    DEFERRED_TASK.store(0, Ordering::Release);
    let controller = CONTROLLER.lock().take();
    let Some(mut controller) = controller else {
        return Ok(());
    };
    crate::serial::write_line(format_args!("[XHCI] shutdown: halting controller"));
    match controller.shutdown_hardware() {
        Ok(()) => {
            crate::serial::write_line(format_args!("[XHCI] shutdown: controller halted"));
            Ok(())
        }
        Err(error) => {
            CONTROLLER.lock().replace(controller);
            Err(error)
        }
    }
}

#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
fn count_inputs(controller: &XhciController) -> (u8, u8) {
    let devices = controller.inputs.iter().flatten();
    // Report-protocol pointers count as mice; other report devices are
    // counted by `input_layouts`.
    devices.fold((0, 0), |(keyboards, mice), device| {
        match (device.kind, device.layout.map(|layout| layout.application)) {
            (HidKind::Keyboard, _) => (keyboards + 1, mice),
            (HidKind::Mouse, _) => (keyboards, mice + 1),
            (_, Some(crate::hid::Application::Mouse | crate::hid::Application::Pointer)) => {
                (keyboards, mice + 1)
            }
            _ => (keyboards, mice),
        }
    })
}

/// The running input delivery, if it has started.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
pub fn input_summary() -> Option<InputSummary> {
    let vector = INPUT_VECTOR.load(Ordering::Acquire);
    if vector == 0 {
        return None;
    }
    let (keyboards, mice) = CONTROLLER.lock().as_ref().map(count_inputs)?;
    Some(InputSummary {
        keyboards,
        mice,
        vector,
    })
}

/// (reports copied, worker passes) since boot.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
pub fn input_counters() -> (u64, u64) {
    (
        INPUT_REPORTS.load(Ordering::Acquire),
        INPUT_PASSES.load(Ordering::Acquire),
    )
}

/// The worker: drain under the lock, publish after it, sleep until the
/// vector fires again.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
fn input_worker() -> ! {
    let mut held = [[0u8; 6]; MAX_INPUTS];
    let mut buttons = [0u8; MAX_INPUTS];
    // Decoders for report-protocol devices, set up once from their layouts:
    // devices are enumerated before the worker starts.
    let mut decoders = [None; MAX_INPUTS];
    if let Some(controller) = CONTROLLER.lock().as_ref() {
        for (decoder, device) in decoders.iter_mut().zip(controller.inputs.iter()) {
            *decoder = device
                .as_ref()
                .and_then(|device| device.layout)
                .map(crate::hid::Decoder::new);
        }
    }
    loop {
        if INPUT_STOP.load(Ordering::Acquire) != 0 {
            INPUT_TASK.store(0, Ordering::Release);
            crate::task::exit_current_task();
        }
        let _ = crate::interrupts::take_pci_device_work(INPUT_VECTOR.load(Ordering::Acquire));
        let mut reports = [(HidKind::Other, 0usize, [0u8; MAX_REPORT]); REPORTS_PER_PASS];
        let count = CONTROLLER
            .lock()
            .as_mut()
            .map_or(0, |controller| controller.service_inputs(&mut reports));
        INPUT_PASSES.fetch_add(1, Ordering::Relaxed);
        for (kind, index, report) in reports[..count].iter().copied() {
            match kind {
                HidKind::Keyboard => {
                    let mut boot = [0u8; 8];
                    boot.copy_from_slice(&report[..8]);
                    for key in keyboard_presses(boot, &mut held[index])
                        .into_iter()
                        .flatten()
                    {
                        let _ = crate::woven_input::publish_key(key);
                    }
                }
                HidKind::Other => {
                    let Some(decoder) = decoders[index].as_mut() else {
                        continue;
                    };
                    let mut inputs = [None; crate::hid::MAX_EVENTS];
                    let count = decoder.decode(&report, &mut inputs);
                    for input in inputs[..count].iter().flatten() {
                        let _ = crate::woven_input::publish(hid_event(*input));
                    }
                }
                HidKind::Mouse => {
                    let mut boot = [0u8; 8];
                    boot.copy_from_slice(&report[..8]);
                    let (events, state) = decode_boot_mouse(boot, buttons[index]);
                    buttons[index] = state;
                    for event in events.into_iter().flatten() {
                        let _ = crate::woven_input::publish(event);
                    }
                }
            }
        }
        // A full pass may have left events behind; drain again first.
        if count < REPORTS_PER_PASS {
            crate::task::wait_for_event();
        }
    }
}

/// A decoded HID input as a WovenInput event.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
fn hid_event(input: crate::hid::Input) -> Event {
    use crate::hid::Input;
    match input {
        Input::Relative { dx, dy } => Event::PointerMove { dx, dy },
        Input::Absolute { x, y } => Event::PointerAbsolute { x, y },
        Input::Wheel(vertical) => Event::Scroll { vertical },
        Input::Button { index, pressed } => Event::PointerButton {
            button: index,
            pressed,
        },
        Input::Pen {
            x,
            y,
            pressure,
            touching,
        } => Event::Pen {
            x,
            y,
            pressure,
            touching,
        },
        Input::Touch {
            contact,
            x,
            y,
            active,
        } => Event::Touch {
            contact,
            x,
            y,
            active,
        },
        Input::Control { control, value } => Event::GameController { control, value },
    }
}

/// Report-protocol devices by application, for the gate.
#[cfg(feature = "stage13-16-test")]
pub fn input_layouts() -> alloc::vec::Vec<crate::hid::Application> {
    CONTROLLER
        .lock()
        .as_ref()
        .map(|controller| {
            controller
                .inputs
                .iter()
                .flatten()
                .filter_map(|device| device.layout.map(|layout| layout.application))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
/// Keys newly pressed in a boot keyboard report, relative to the keys held
/// in the previous one (updated). Held keys do not repeat; released keys
/// produce nothing; usages WovenInput has no key for are ignored.
pub fn keyboard_presses(report: [u8; 8], held: &mut [u8; 6]) -> [Option<Key>; 6] {
    let shift = report[0] & 0x22 != 0;
    let mut keys = [None; 6];
    let mut current = [0u8; 6];
    current.copy_from_slice(&report[2..8]);
    // 0x01 in every slot is the keyboard's rollover-error report: no keys.
    if current.iter().all(|usage| *usage == 1) {
        return keys;
    }
    for (slot, usage) in keys.iter_mut().zip(current) {
        if usage > 3 && !held.contains(&usage) {
            *slot = usage_key(usage, shift);
        }
    }
    *held = current;
    keys
}

/// A HID keyboard usage (page 7) as a WovenInput key.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
fn usage_key(usage: u8, shift: bool) -> Option<Key> {
    const DIGITS: &[u8; 10] = b"1234567890";
    const SHIFTED_DIGITS: &[u8; 10] = b"!@#$%^&*()";
    // Usages 0x2d-0x38: - = [ ] \ (non-US #) ; ' ` , . /
    const PUNCTUATION: &[u8; 12] = b"-=[]\\#;'`,./";
    const SHIFTED_PUNCTUATION: &[u8; 12] = b"_+{}|~:\"~<>?";
    let character = match usage {
        0x04..=0x1d => {
            let letter = b'a' + (usage - 0x04);
            if shift {
                letter.to_ascii_uppercase()
            } else {
                letter
            }
        }
        0x1e..=0x27 => {
            let index = usize::from(usage - 0x1e);
            if shift {
                SHIFTED_DIGITS[index]
            } else {
                DIGITS[index]
            }
        }
        0x28 => return Some(Key::Enter),
        0x2a => return Some(Key::Backspace),
        0x2b => return Some(Key::Tab),
        0x2c => b' ',
        0x2d..=0x38 => {
            let index = usize::from(usage - 0x2d);
            if shift {
                SHIFTED_PUNCTUATION[index]
            } else {
                PUNCTUATION[index]
            }
        }
        0x3a => return Some(Key::F1),
        0x3b => return Some(Key::F2),
        0x3c => return Some(Key::F3),
        0x3d => return Some(Key::F4),
        0x45 => return Some(Key::F12),
        _ => return None,
    };
    Some(Key::Char(char::from(character)))
}

pub fn init() -> Result<(u8, u8, u8, u8), InitError> {
    if let Some(summary) = CONTROLLER.lock().as_ref().map(XhciController::summary) {
        return Ok(summary);
    }
    init_at(find_controller().ok_or(InitError::MissingController)?)
}

/// Initialize the controller at `device`, the function its driver is bound
/// to. An already initialized controller is kept.
pub fn init_at(device: pci::Device) -> Result<(u8, u8, u8, u8), InitError> {
    let mut slot = CONTROLLER.lock();
    if let Some(controller) = slot.as_ref() {
        return Ok(controller.summary());
    }
    let controller = XhciController::initialize(device)?;
    let summary = controller.summary();
    *slot = Some(controller);
    Ok(summary)
}

pub fn init_hid() -> Result<HidSummary, InitError> {
    let mut slot = CONTROLLER.lock();
    if slot.is_none() {
        let device = find_controller().ok_or(InitError::MissingController)?;
        *slot = Some(XhciController::initialize(device)?);
    }
    slot.as_mut()
        .ok_or(InitError::MissingController)?
        .enumerate_hid()
}

pub fn init_bluetooth() -> Result<BluetoothUsbInterface, InitError> {
    let mut slot = CONTROLLER.lock();
    if slot.is_none() {
        let device = find_controller().ok_or(InitError::MissingController)?;
        *slot = Some(XhciController::initialize(device)?);
    }
    slot.as_mut()
        .ok_or(InitError::MissingController)?
        .enumerate_bluetooth()
}

pub fn send_bluetooth_command(command: &[u8]) -> Result<(), InitError> {
    CONTROLLER
        .lock()
        .as_mut()
        .ok_or(InitError::MissingController)?
        .send_bluetooth_command(command)
}

pub fn receive_bluetooth_event() -> Result<[u8; 257], InitError> {
    CONTROLLER
        .lock()
        .as_mut()
        .ok_or(InitError::MissingController)?
        .receive_bluetooth_event()
}

pub fn receive_bluetooth_acl() -> Result<[u8; PAGE_SIZE], InitError> {
    CONTROLLER
        .lock()
        .as_mut()
        .ok_or(InitError::MissingController)?
        .receive_bluetooth_acl()
}

pub fn send_bluetooth_acl(data: &[u8]) -> Result<(), InitError> {
    CONTROLLER
        .lock()
        .as_mut()
        .ok_or(InitError::MissingController)?
        .send_bluetooth_acl(data)
}

pub fn poll_hid_report() -> Result<[u8; 8], InitError> {
    CONTROLLER
        .lock()
        .as_mut()
        .ok_or(InitError::MissingController)?
        .poll_hid_report()
}

pub fn probe() -> bool {
    find_controller().is_some()
}

/// Read-only controller inventory for the physical diagnostic image. This
/// deliberately does not enable PCI decoding, start/reset the controller,
/// allocate DMA, or write any MMIO register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadOnlyDiagnostic {
    pub bar: u64,
    pub cap_length: u8,
    pub max_slots: u8,
    pub max_ports: u8,
    pub context_64: bool,
    pub doorbell_offset: u32,
    pub runtime_offset: u32,
    pub command: u32,
    pub status: u32,
    pub config: u32,
    pub connected_ports: u8,
    pub enabled_ports: u8,
    pub port_status: [u32; 16],
    pub port_count: u8,
}

pub fn read_only_diagnostic() -> Result<ReadOnlyDiagnostic, InitError> {
    let device = find_controller().ok_or(InitError::MissingController)?;
    let bar = device
        .bars
        .iter()
        .find(|bar| bar.valid && matches!(bar.kind, BarKind::Memory32 | BarKind::Memory64))
        .ok_or(InitError::MissingMemoryBar)?;
    let mmio = bar.address;
    let cap_length = mmio_read8(mmio)?;
    if cap_length < 0x20 {
        return Err(InitError::InvalidRegisters);
    }
    let hcs1 = mmio_read32(mmio + 0x04)?;
    let hcc1 = mmio_read32(mmio + 0x10)?;
    let max_slots = (hcs1 & 0xff) as u8;
    let max_ports = ((hcs1 >> 24) & 0xff) as u8;
    let port_count = max_ports.min(16);
    if max_slots == 0 || max_ports == 0 {
        return Err(InitError::InvalidRegisters);
    }
    let op_base = mmio + u64::from(cap_length);
    let command = mmio_read32(op_base)?;
    let status = mmio_read32(op_base + 0x04)?;
    let config = mmio_read32(op_base + 0x38)?;
    let doorbell_offset = mmio_read32(mmio + 0x14)?;
    let runtime_offset = mmio_read32(mmio + 0x18)?;
    let mut port_status = [0u32; 16];
    let mut connected_ports = 0;
    let mut enabled_ports = 0;
    for port in 1..=port_count {
        let value = mmio_read32(op_base + 0x400 + (u64::from(port - 1) * 0x10))?;
        port_status[usize::from(port - 1)] = value;
        if value & 1 != 0 {
            connected_ports += 1;
        }
        if value & (1 << 1) != 0 {
            enabled_ports += 1;
        }
    }
    Ok(ReadOnlyDiagnostic {
        bar: mmio,
        cap_length,
        max_slots,
        max_ports,
        context_64: hcc1 & (1 << 2) != 0,
        doorbell_offset,
        runtime_offset,
        command,
        status,
        config,
        connected_ports,
        enabled_ports,
        port_status,
        port_count,
    })
}

fn find_controller() -> Option<pci::Device> {
    for index in 0..64 {
        if let Some(device) = pci::device(index) {
            if device.class == USB_CLASS
                && device.subclass == USB_SUBCLASS
                && device.prog_if == XHCI_PROG_IF
            {
                return Some(device);
            }
        }
    }
    None
}

fn endpoint_dci(endpoint_address: u8) -> Result<u8, InitError> {
    let number = endpoint_address & 0x0f;
    if number == 0 {
        return Err(InitError::DescriptorInvalid);
    }
    let direction_in = endpoint_address & 0x80 != 0;
    let dci = number
        .saturating_mul(2)
        .saturating_add(u8::from(direction_in));
    if dci > 31 {
        return Err(InitError::DescriptorInvalid);
    }
    Ok(dci)
}

#[allow(clippy::too_many_arguments)]
fn write_bluetooth_endpoint_context(
    input: &mut [u8; PAGE_SIZE],
    context_size: usize,
    dci: u8,
    endpoint_address: u8,
    max_packet: u16,
    interval: u8,
    speed: u8,
    ring_physical: u64,
    transfer_type: u8,
) -> Result<(), InitError> {
    if max_packet == 0 {
        return Err(InitError::DescriptorInvalid);
    }
    let direction_in = endpoint_address & 0x80 != 0;
    let endpoint_type = match (transfer_type, direction_in) {
        (USB_ENDPOINT_INTERRUPT, true) => 7_u32,
        (USB_ENDPOINT_BULK, true) => 6_u32,
        (USB_ENDPOINT_BULK, false) => 2_u32,
        _ => return Err(InitError::DescriptorInvalid),
    };
    let offset = context_size * (usize::from(dci) + 1);
    let interval_value = if transfer_type == USB_ENDPOINT_INTERRUPT {
        xhci_interval(speed, interval)
    } else {
        0
    };
    write_u32(input, offset, u32::from(interval_value) << 16);
    write_u32(
        input,
        offset + 4,
        (3 << 1) | (endpoint_type << 3) | (u32::from(max_packet) << 16),
    );
    write_u64(input, offset + 8, ring_physical | 1);
    write_u32(
        input,
        offset + 16,
        u32::from(max_packet) | (u32::from(max_packet) << 16),
    );
    Ok(())
}

fn find_connected_port(op_base: u64, max_ports: u8) -> Result<u8, InitError> {
    for port in 1..=max_ports {
        let portsc = mmio_read32(op_base + 0x400 + (u64::from(port - 1) * 0x10))?;
        if portsc & 1 != 0 {
            return Ok(port);
        }
    }
    Err(InitError::NoConnectedPort)
}

fn parse_hid_interface(bytes: &[u8], configuration: u8) -> Option<HidInterface> {
    let mut offset = 0usize;
    let mut current: Option<(u8, u8)> = None;
    let mut report_length = 0u16;
    while offset + 2 <= bytes.len() {
        let length = usize::from(bytes[offset]);
        if length < 2 || offset + length > bytes.len() {
            return None;
        }
        match bytes[offset + 1] {
            4 if length >= 9 => {
                if bytes[offset + 5] == USB_CLASS_HID {
                    current = Some((bytes[offset + 2], bytes[offset + 7]));
                } else {
                    current = None;
                }
                report_length = 0;
            }
            // HID class descriptor: the report descriptor's length.
            0x21 if length >= 9 && current.is_some() && bytes[offset + 6] == 0x22 => {
                report_length = u16::from_le_bytes([bytes[offset + 7], bytes[offset + 8]]);
            }
            5 if length >= 7 => {
                if let Some((interface, protocol)) = current {
                    let address = bytes[offset + 2];
                    let attributes = bytes[offset + 3] & 0x03;
                    if address & 0x80 != 0 && attributes == 0x03 {
                        let max_packet =
                            u16::from_le_bytes([bytes[offset + 4], bytes[offset + 5]]) & 0x07ff;
                        if max_packet != 0 {
                            return Some(HidInterface {
                                interface,
                                configuration,
                                endpoint_address: address,
                                max_packet,
                                interval: bytes[offset + 6],
                                protocol,
                                report_length,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
        offset += length;
    }
    None
}

pub fn parse_bluetooth_usb_interface(
    bytes: &[u8],
    configuration: u8,
) -> Option<BluetoothUsbInterface> {
    let mut offset = 0usize;
    let mut interface = None;
    let mut event = None;
    let mut acl_in = None;
    let mut acl_out = None;

    while offset + 2 <= bytes.len() {
        let length = usize::from(bytes[offset]);
        if length < 2 || offset + length > bytes.len() {
            return None;
        }
        match bytes[offset + 1] {
            4 if length >= 9 => {
                let matches_bluetooth = bytes[offset + 5] == USB_CLASS_WIRELESS_CONTROLLER
                    && bytes[offset + 6] == USB_SUBCLASS_RF_CONTROLLER
                    && bytes[offset + 7] == USB_PROTOCOL_BLUETOOTH_PRIMARY;
                if matches_bluetooth {
                    interface = Some(bytes[offset + 2]);
                    event = None;
                    acl_in = None;
                    acl_out = None;
                } else {
                    interface = None;
                }
            }
            5 if length >= 7 && interface.is_some() => {
                let address = bytes[offset + 2];
                let transfer_type = bytes[offset + 3] & 0x03;
                let max_packet =
                    u16::from_le_bytes([bytes[offset + 4], bytes[offset + 5]]) & 0x07ff;
                if max_packet != 0 {
                    match (transfer_type, address & 0x80 != 0) {
                        (USB_ENDPOINT_INTERRUPT, true) => {
                            event = Some((address, max_packet, bytes[offset + 6]));
                        }
                        (USB_ENDPOINT_BULK, true) => acl_in = Some((address, max_packet)),
                        (USB_ENDPOINT_BULK, false) => acl_out = Some((address, max_packet)),
                        _ => {}
                    }
                }
                if let (
                    Some(interface),
                    Some((event_endpoint, event_max_packet, event_interval)),
                    Some((acl_in_endpoint, acl_in_max_packet)),
                    Some((acl_out_endpoint, acl_out_max_packet)),
                ) = (interface, event, acl_in, acl_out)
                {
                    return Some(BluetoothUsbInterface {
                        interface,
                        configuration,
                        event_endpoint,
                        event_max_packet,
                        event_interval,
                        acl_in_endpoint,
                        acl_in_max_packet,
                        acl_out_endpoint,
                        acl_out_max_packet,
                    });
                }
            }
            _ => {}
        }
        offset += length;
    }
    None
}

pub fn bluetooth_command_setup(interface: u8, command_len: u16) -> u64 {
    setup_packet(
        USB_REQUEST_TYPE_BLUETOOTH_COMMAND,
        0,
        0,
        u16::from(interface),
        command_len,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BluetoothTransferKind {
    Command,
    Event,
    AclIn,
    AclOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BluetoothTransferPlan {
    pub kind: BluetoothTransferKind,
    pub endpoint: u8,
    pub length: u16,
}

pub fn bluetooth_command_plan(
    interface: BluetoothUsbInterface,
    length: usize,
) -> Option<BluetoothTransferPlan> {
    if length == 0 || length > 258 {
        return None;
    }
    Some(BluetoothTransferPlan {
        kind: BluetoothTransferKind::Command,
        endpoint: interface.interface,
        length: length as u16,
    })
}

pub fn bluetooth_event_plan(interface: BluetoothUsbInterface) -> BluetoothTransferPlan {
    BluetoothTransferPlan {
        kind: BluetoothTransferKind::Event,
        endpoint: interface.event_endpoint,
        length: interface.event_max_packet,
    }
}

pub fn bluetooth_acl_in_plan(interface: BluetoothUsbInterface) -> BluetoothTransferPlan {
    BluetoothTransferPlan {
        kind: BluetoothTransferKind::AclIn,
        endpoint: interface.acl_in_endpoint,
        length: interface.acl_in_max_packet,
    }
}

pub fn bluetooth_acl_out_plan(
    interface: BluetoothUsbInterface,
    length: usize,
) -> Option<BluetoothTransferPlan> {
    if length == 0 || length > usize::from(interface.acl_out_max_packet) {
        return None;
    }
    Some(BluetoothTransferPlan {
        kind: BluetoothTransferKind::AclOut,
        endpoint: interface.acl_out_endpoint,
        length: length as u16,
    })
}

pub fn bluetooth_transport_self_test() -> bool {
    let descriptor = [
        9, 2, 39, 0, 1, 1, 0, 0x80, 50, 9, 4, 0, 0, 3, 0xe0, 0x01, 0x01, 0, 7, 5, 0x81, 0x03, 16,
        0, 1, 7, 5, 0x82, 0x02, 64, 0, 0, 7, 5, 0x02, 0x02, 64, 0, 0,
    ];
    let Some(bt) = parse_bluetooth_usb_interface(&descriptor, 1) else {
        return false;
    };
    bt.interface == 0
        && bt.configuration == 1
        && bt.event_endpoint == 0x81
        && bt.event_max_packet == 16
        && bt.event_interval == 1
        && bt.acl_in_endpoint == 0x82
        && bt.acl_in_max_packet == 64
        && bt.acl_out_endpoint == 0x02
        && bt.acl_out_max_packet == 64
        && bluetooth_command_setup(0, 3) == setup_packet(0x20, 0, 0, 0, 3)
        && bluetooth_command_plan(bt, 3)
            == Some(BluetoothTransferPlan {
                kind: BluetoothTransferKind::Command,
                endpoint: 0,
                length: 3,
            })
        && bluetooth_event_plan(bt).endpoint == 0x81
        && bluetooth_acl_in_plan(bt).endpoint == 0x82
        && bluetooth_acl_out_plan(bt, 32).map(|plan| plan.endpoint) == Some(0x02)
        && bluetooth_acl_out_plan(bt, 65).is_none()
        && live_bluetooth_command_contract_self_test(bt)
}

fn live_bluetooth_command_contract_self_test(bluetooth: BluetoothUsbInterface) -> bool {
    let command = [0x03_u8, 0x0c, 0x00];
    let Some(plan) = bluetooth_command_plan(bluetooth, command.len()) else {
        return false;
    };
    plan.kind == BluetoothTransferKind::Command
        && plan.endpoint == bluetooth.interface
        && plan.length == 3
        && bluetooth_command_setup(bluetooth.interface, plan.length)
            == setup_packet(
                USB_REQUEST_TYPE_BLUETOOTH_COMMAND,
                0,
                0,
                u16::from(bluetooth.interface),
                3,
            )
}

fn hid_kind(protocol: u8) -> HidKind {
    match protocol {
        USB_PROTOCOL_KEYBOARD => HidKind::Keyboard,
        USB_PROTOCOL_MOUSE => HidKind::Mouse,
        _ => HidKind::Other,
    }
}

fn xhci_interval(speed: u8, usb_interval: u8) -> u8 {
    let interval = usb_interval.max(1);
    if speed >= 3 {
        interval.saturating_sub(1).min(15)
    } else {
        let mut frames = u16::from(interval);
        let mut exponent = 0_u8;
        while frames > 1 {
            frames = frames.div_ceil(2);
            exponent = exponent.saturating_add(1);
        }
        exponent.saturating_add(3).min(15)
    }
}

fn setup_packet(request_type: u8, request: u8, value: u16, index: u16, length: u16) -> u64 {
    u64::from(request_type)
        | (u64::from(request) << 8)
        | (u64::from(value) << 16)
        | (u64::from(index) << 32)
        | (u64::from(length) << 48)
}

pub fn decode_boot_keyboard(report: [u8; 8]) -> Option<u8> {
    report[2..8].iter().copied().find(|key| *key != 0)
}

/// Decode the fixed boot-protocol mouse report without retaining a pointer to
/// controller/DMA memory. The caller owns the previous button state and must
/// publish the returned events after releasing any controller lock.
pub fn decode_boot_mouse(report: [u8; 8], previous_buttons: u8) -> ([Option<Event>; 4], u8) {
    let mut events = [None; 4];
    let buttons = report[0] & 0x07;
    // HID boot mice report Y positive towards the user, i.e. down the
    // screen, which is already WovenInput's orientation (PS/2 is the one
    // that reports up as positive).
    let dx = i16::from(report[1] as i8);
    let dy = i16::from(report[2] as i8);
    let mut count = 0;
    if dx != 0 || dy != 0 {
        events[count] = Some(Event::PointerMove { dx, dy });
        count += 1;
    }
    for button in 0..3u8 {
        let mask = 1 << button;
        if (buttons ^ previous_buttons) & mask != 0 {
            events[count] = Some(Event::PointerButton {
                button,
                pressed: buttons & mask != 0,
            });
            count += 1;
        }
    }
    (events, buttons)
}

/// Stage 13.13 acceptance: USB keyboard and mouse input. The harness types
/// and moves through QEMU's own USB keyboard and mouse (QMP, addressed to
/// those devices) once this prints its ready line; the events must arrive in
/// WovenInput exactly, and through interrupts rather than polling.
#[cfg(feature = "stage13-13-test")]
pub fn input_self_test() -> bool {
    use crate::woven_input;
    use alloc::vec::Vec;

    fn step(name: &str, ok: bool) -> bool {
        crate::serial::write_line(format_args!(
            "[S13.13] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    // The decoder first: presses, no repeats while held, shift, rollover.
    let mut held = [0u8; 6];
    let press = keyboard_presses([0, 0, 0x04, 0, 0, 0, 0, 0], &mut held);
    let repeat = keyboard_presses([0, 0, 0x04, 0, 0, 0, 0, 0], &mut held);
    let second = keyboard_presses([0, 0, 0x04, 0x28, 0, 0, 0, 0], &mut held);
    let released = keyboard_presses([0; 8], &mut held);
    let shifted = keyboard_presses([0x02, 0, 0x1e, 0, 0, 0, 0, 0], &mut held);
    let before_rollover = held;
    let rollover = keyboard_presses([0, 0, 1, 1, 1, 1, 1, 1], &mut held);
    let decoding = press[0] == Some(Key::Char('a'))
        && press[1..].iter().all(Option::is_none)
        && repeat.iter().all(Option::is_none)
        && second[0].is_none()
        && second[1] == Some(Key::Enter)
        && released.iter().all(Option::is_none)
        && shifted[0] == Some(Key::Char('!'))
        && rollover.iter().all(Option::is_none)
        && held == before_rollover;
    if !step(
        "keyboard reports: presses once, shift, rollover ignored",
        decoding,
    ) {
        return false;
    }

    let report = crate::drivers::bring_up();
    let xhci = report.drivers[2];
    if !step(
        "xhci0 bound and ready",
        matches!(xhci.outcome, crate::drivers::Outcome::Ready(_)),
    ) {
        return false;
    }
    let Some(summary) = input_summary() else {
        return step("input delivery started", false);
    };
    let started = summary.keyboards == 1
        && summary.mice == 1
        && (crate::interrupts::PCI_VECTOR_FIRST..=crate::interrupts::PCI_VECTOR_LAST)
            .contains(&summary.vector);
    if !step(
        "one keyboard and one mouse, on a device interrupt vector",
        started,
    ) {
        return false;
    }
    // Nothing queued before the input arrives.
    while woven_input::poll().is_some() {}
    let irqs_before = crate::interrupts::pci_device_irq_count(summary.vector);
    let (reports_before, passes_before) = input_counters();
    crate::serial::write_line(format_args!("[S13.13] ready for input"));

    let expected = [
        Key::Char('w'),
        Key::Char('h'),
        Key::Char('1'),
        Key::Char('A'),
        Key::Enter,
    ];
    let mut keys = Vec::new();
    let mut motion = (0i32, 0i32);
    let mut clicks = Vec::new();
    let start = crate::timer::ticks();
    while crate::timer::ticks().wrapping_sub(start) < 6000 {
        while let Some(event) = woven_input::poll() {
            match event {
                woven_input::Event::Key(key) => keys.push(key),
                woven_input::Event::PointerMove { dx, dy } => {
                    motion.0 += i32::from(dx);
                    motion.1 += i32::from(dy);
                }
                woven_input::Event::PointerButton { button, pressed } => {
                    clicks.push((button, pressed))
                }
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }
        if keys.len() >= expected.len() && clicks.len() >= 2 && motion == (40, 25) {
            break;
        }
        crate::task::yield_now();
    }
    crate::serial::write_line(format_args!(
        "[S13.13] received keys={:?} motion={:?} clicks={:?}",
        keys, motion, clicks
    ));
    let keys_ok = step(
        "typed keys arrive once each, in order, shift applied",
        keys == expected,
    );
    let mouse_ok = step(
        "mouse motion (right 40, down 25) and a left click arrive",
        motion == (40, 25) && clicks == [(0, true), (0, false)],
    );
    let irqs = crate::interrupts::pci_device_irq_count(summary.vector) - irqs_before;
    let (reports, passes) = input_counters();
    let (reports, passes) = (reports - reports_before, passes - passes_before);
    crate::serial::write_line(format_args!(
        "[S13.13] interrupts={} reports={} worker passes={}",
        irqs, reports, passes
    ));
    // Every pass but a trailing one follows an interrupt: the worker sleeps
    // between them instead of polling.
    let event_driven = step(
        "delivered by device interrupts; the worker sleeps between them",
        irqs > 0 && reports >= 5 && passes <= irqs + 2,
    );
    keys_ok && mouse_ok && event_driven
}

/// Stage 13.18 acceptance: a USB mass-storage disk.
///
/// QEMU attaches a 32 MiB raw image as `usb-storage` beside a USB keyboard.
/// Every sector of the image starts with `WOVEN-LBA-` and its LBA in eight
/// decimal digits. The kernel must identify the disk and its capacity, read
/// the pattern back, write 64 sectors and read them back after a flush, and
/// refuse a block past the end. It then reads 300 more blocks, which wraps
/// both bulk rings several times, while the harness types on the USB
/// keyboard: the keys must arrive, in order, though storage transfers drain
/// the event ring they arrive on. The harness requires exactly the written
/// sectors to have changed in the image.
#[cfg(feature = "stage13-18-test")]
pub fn storage_self_test() -> bool {
    use crate::block::{BlockDevice, SECTOR_SIZE};
    use crate::woven_input;
    use alloc::vec::Vec;

    fn step(name: &str, ok: bool) -> bool {
        crate::serial::write_line(format_args!(
            "[S13.18] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn pattern(lba: u64) -> [u8; 18] {
        let mut text = *b"WOVEN-LBA-00000000";
        let mut value = lba;
        for digit in text[10..].iter_mut().rev() {
            *digit = b'0' + (value % 10) as u8;
            value /= 10;
        }
        text
    }
    const BLOCKS: u64 = 32 * 1024 * 1024 / 512;
    const WRITE_BASE: u64 = 4096;
    const WRITTEN: u64 = 64;

    let report = crate::drivers::bring_up();
    if !step(
        "xhci0 bound and ready",
        matches!(report.drivers[2].outcome, crate::drivers::Outcome::Ready(_)),
    ) {
        return false;
    }
    let Some(info) = usb_disk_info() else {
        return step("a USB disk is attached", false);
    };
    crate::serial::write_line(format_args!(
        "[S13.18] disk on port {}: '{}' '{}', {} blocks",
        info.port,
        core::str::from_utf8(&info.vendor).unwrap_or("?").trim_end(),
        core::str::from_utf8(&info.product)
            .unwrap_or("?")
            .trim_end(),
        info.blocks
    ));
    if !step(
        "identified as QEMU's disk, 32 MiB of 512-byte blocks",
        &info.vendor[..4] == b"QEMU" && info.blocks == BLOCKS,
    ) {
        return false;
    }
    let mut disk = UsbDisk;
    let mut sector = [0u8; SECTOR_SIZE];
    let reads = (0..8u64)
        .chain([BLOCKS - 1])
        .all(|lba| disk.read_sector(lba, &mut sector).is_ok() && sector[..18] == pattern(lba));
    if !step(
        "the host's pattern reads back, first and last blocks",
        reads,
    ) {
        return false;
    }
    let mut writes = true;
    for lba in WRITE_BASE..WRITE_BASE + WRITTEN {
        let mut data = [0u8; SECTOR_SIZE];
        data[..13].copy_from_slice(b"S13.18-WRITE-");
        data[13..21].copy_from_slice(&lba.to_le_bytes());
        data[SECTOR_SIZE - 1] = lba as u8;
        writes &= disk.write_sector(lba, &data).is_ok();
    }
    writes &= disk.flush().is_ok();
    let readback = (WRITE_BASE..WRITE_BASE + WRITTEN).all(|lba| {
        disk.read_sector(lba, &mut sector).is_ok()
            && &sector[..13] == b"S13.18-WRITE-"
            && sector[13..21] == lba.to_le_bytes()
            && sector[SECTOR_SIZE - 1] == lba as u8
    });
    if !step(
        "64 blocks written, flushed and read back",
        writes && readback,
    ) {
        return false;
    }
    if !step(
        "a block past the end is refused",
        disk.read_sector(BLOCKS, &mut sector).is_err(),
    ) {
        return false;
    }

    let Some(summary) = input_summary() else {
        return step("keyboard input started", false);
    };
    if !step("a USB keyboard beside the disk", summary.keyboards == 1) {
        return false;
    }
    while woven_input::poll().is_some() {}
    crate::serial::write_line(format_args!("[S13.18] ready for input"));
    let expected = [Key::Char('u'), Key::Char('s'), Key::Char('b'), Key::Enter];
    let mut keys = Vec::new();
    let mut verified = 0u64;
    let start = crate::timer::ticks();
    let mut lba = 100u64;
    // Storage I/O runs throughout: the keys arrive while transfers drain the
    // event ring.
    while crate::timer::ticks().wrapping_sub(start) < 6000
        && (keys.len() < expected.len() || verified < 300)
    {
        if disk.read_sector(lba, &mut sector).is_ok() && sector[..18] == pattern(lba) {
            verified += 1;
        }
        lba = if lba >= 3000 { 100 } else { lba + 1 };
        while let Some(event) = woven_input::poll() {
            if let woven_input::Event::Key(key) = event {
                keys.push(key);
            }
        }
        crate::task::yield_now();
    }
    crate::serial::write_line(format_args!(
        "[S13.18] verified {} reads; keys {:?}",
        verified, keys
    ));
    let storage_ok = step(
        "300 more reads verify, wrapping both bulk rings",
        verified >= 300,
    );
    let keys_ok = step(
        "typed keys arrive once each, in order, during storage I/O",
        keys == expected,
    );
    storage_ok && keys_ok
}

/// Stage 13.16 acceptance: a report-protocol HID device. QEMU's usb-tablet
/// describes its reports in a report descriptor (absolute X/Y, buttons,
/// wheel). Bring-up must parse it, and positions, a click and a wheel step
/// injected over QMP must arrive in WovenInput exactly, carried by device
/// interrupts.
#[cfg(feature = "stage13-16-test")]
pub fn report_device_self_test() -> bool {
    use crate::woven_input;
    use alloc::vec::Vec;

    fn step(name: &str, ok: bool) -> bool {
        crate::serial::write_line(format_args!(
            "[S13.16] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    let report = crate::drivers::bring_up();
    if !step(
        "xhci0 bound and ready",
        matches!(report.drivers[2].outcome, crate::drivers::Outcome::Ready(_)),
    ) {
        return false;
    }
    let layouts = input_layouts();
    crate::serial::write_line(format_args!(
        "[S13.16] report-protocol devices: {:?}",
        layouts
    ));
    let Some(summary) = input_summary() else {
        return step("input delivery started", false);
    };
    if !step(
        "the tablet's report descriptor parses as a pointer",
        layouts == [crate::hid::Application::Mouse] && summary.mice == 1,
    ) {
        return false;
    }
    while woven_input::poll().is_some() {}
    let irqs_before = crate::interrupts::pci_device_irq_count(summary.vector);
    crate::serial::write_line(format_args!("[S13.16] ready for input"));

    let mut positions = Vec::new();
    let mut clicks = Vec::new();
    let mut scrolls = Vec::new();
    let start = crate::timer::ticks();
    while crate::timer::ticks().wrapping_sub(start) < 6000 {
        while let Some(event) = woven_input::poll() {
            match event {
                woven_input::Event::PointerAbsolute { x, y } => positions.push((x, y)),
                woven_input::Event::PointerButton { button, pressed } => {
                    clicks.push((button, pressed))
                }
                woven_input::Event::Scroll { vertical } => scrolls.push(vertical),
                _ => {}
            }
        }
        if positions.contains(&(24_576, 8_192)) && clicks.len() >= 2 && scrolls.len() >= 2 {
            break;
        }
        crate::task::yield_now();
    }
    crate::serial::write_line(format_args!(
        "[S13.16] received positions={:?} clicks={:?} scrolls={:?}",
        positions, clicks, scrolls
    ));
    // In order, X before Y: a swapped axis or a wrong scale moves them.
    let moved = step(
        "absolute positions arrive scaled to the full range, in order",
        positions.first() == Some(&(8_192, 24_576)) && positions[1..].contains(&(24_576, 8_192)),
    );
    let clicked = step(
        "a left click arrives as press then release",
        clicks == [(0, true), (0, false)],
    );
    let scrolled = step("a wheel step up then down arrives", scrolls == [1, -1]);
    let irqs = crate::interrupts::pci_device_irq_count(summary.vector) - irqs_before;
    let interrupt = step("delivered by device interrupts", irqs > 0);
    moved && clicked && scrolled && interrupt
}

pub fn self_test() -> bool {
    let descriptor = [
        9, 2, 25, 0, 1, 1, 0, 0x80, 50, 9, 4, 0, 0, 1, 3, 1, 1, 0, 7, 5, 0x81, 3, 8, 0, 10,
    ];
    let hid = parse_hid_interface(&descriptor, 1);
    let (mouse_events, mouse_buttons) = decode_boot_mouse([0x01, 5, 0xfd, 0, 0, 0, 0, 0], 0);
    let mouse_path = mouse_events[0] == Some(Event::PointerMove { dx: 5, dy: -3 })
        && mouse_events[1]
            == Some(Event::PointerButton {
                button: 0,
                pressed: true,
            })
        && mouse_buttons == 1;
    let (release_events, release_buttons) = decode_boot_mouse([0, 0, 0, 0, 0, 0, 0, 0], 1);
    let mouse_release = release_events[0]
        == Some(Event::PointerButton {
            button: 0,
            pressed: false,
        })
        && release_buttons == 0;
    trb_type(TRB_TYPE_ENABLE_SLOT << 10) == TRB_TYPE_ENABLE_SLOT
        && completion_code(1 << 24) == 1
        && trb_type(TRB_TYPE_PORT_STATUS_CHANGE << 10) == TRB_TYPE_PORT_STATUS_CHANGE
        && hid
            .map(|entry| entry.protocol == USB_PROTOCOL_KEYBOARD && entry.endpoint_address == 0x81)
            == Some(true)
        && decode_boot_keyboard([0, 0, 4, 0, 0, 0, 0, 0]) == Some(4)
        && mouse_path
        && mouse_release
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn trb_type(control: u32) -> u32 {
    (control >> 10) & 0x3f
}
fn completion_code(status: u32) -> u8 {
    (status >> 24) as u8
}

fn wait_until(address: u64, mask: u32, expected: u32) -> Result<(), InitError> {
    for _ in 0..POLL_LIMIT {
        if mmio_read32(address)? & mask == expected {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(InitError::ControllerTimeout)
}

fn mmio_read8(physical: u64) -> Result<u8, InitError> {
    let page = physical & !0xfff;
    let offset = physical & 0xfff;
    let mapped = paging::map_mmio(page).map_err(|_| InitError::MmioUnavailable)?;
    // SAFETY: map_mmio establishes a mapping for this controller register page.
    Ok(unsafe { ptr::read_volatile((mapped + offset) as *const u8) })
}

fn mmio_read32(physical: u64) -> Result<u32, InitError> {
    let page = physical & !0xfff;
    let offset = physical & 0xfff;
    let mapped = paging::map_mmio(page).map_err(|_| InitError::MmioUnavailable)?;
    // SAFETY: map_mmio establishes a mapping for this controller register page.
    Ok(unsafe { ptr::read_volatile((mapped + offset) as *const u32) })
}

fn mmio_write32(physical: u64, value: u32) -> Result<(), InitError> {
    let page = physical & !0xfff;
    let offset = physical & 0xfff;
    let mapped = paging::map_mmio(page).map_err(|_| InitError::MmioUnavailable)?;
    // SAFETY: map_mmio establishes a mapping for this controller register page.
    unsafe { ptr::write_volatile((mapped + offset) as *mut u32, value) };
    Ok(())
}

fn mmio_write64(physical: u64, value: u64) -> Result<(), InitError> {
    mmio_write32(physical, value as u32)?;
    mmio_write32(physical + 4, (value >> 32) as u32)
}
