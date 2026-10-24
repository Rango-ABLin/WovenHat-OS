//! Read-only PCI inventory boot mode for the selected physical machine.
//! Reports to framebuffer and serial, then halts before device activation.

#[cfg(feature = "physical-wifi")]
use crate::wifi::Manager;
use crate::{console::Console, hal, serial};
use core::fmt::{self, Write};
#[cfg(feature = "physical-wifi")]
use sha2::{Digest, Sha256};

struct Screen<'a>(&'a mut Console);
impl Write for Screen<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        // The console font lacks square brackets. Keep serial markers exact
        // while displaying the same labels without unsupported glyphs.
        for part in text.split(['[', ']']) {
            self.0.print(part);
        }
        Ok(())
    }
}

fn line(console: &mut Console, args: fmt::Arguments<'_>) {
    serial::write_line(args);
    let _ = Screen(console).write_fmt(args);
    console.println("");
}

/// Read only the first Intel CSR words from an already memory-decoded AX201
/// BAR. This deliberately does not enable PCI memory space, bus mastering,
/// MSI, reset, DMA or any device-owned queue. The physical probe uses this to
/// establish register visibility before the real driver is allowed to bind.
#[cfg(not(feature = "physical-wifi"))]
fn read_ax201_csrs(device: &hal::pci::Device) -> Result<[u32; 7], &'static str> {
    if device.command & 0x2 == 0 {
        return Err("MEMORY_DISABLED");
    }
    let bar = device
        .bars
        .iter()
        .find(|bar| {
            bar.valid
                && matches!(
                    bar.kind,
                    hal::pci::BarKind::Memory32 | hal::pci::BarKind::Memory64
                )
                && bar.address != 0
        })
        .ok_or("NO_MMIO_BAR")?;
    let page = bar.address & !0xfff;
    let base = crate::paging::map_mmio(page).map_err(|_| "MMIO_MAP_FAILED")?;
    let bar_offset = bar.address - page;
    let offsets = [0usize, 0x08, 0x0c, 0x20, 0x24, 0x28, 0x50];
    let mut values = [0u32; 7];
    for (index, offset) in offsets.into_iter().enumerate() {
        let address = base
            .checked_add(bar_offset)
            .and_then(|value| value.checked_add(offset as u64))
            .ok_or("MMIO_ADDRESS_OVERFLOW")?;
        // SAFETY: PCI memory decoding is already enabled, the offset is within
        // the first 0x54 bytes of the discovered BAR, and this probe never
        // aliases the mapping as ordinary Rust memory or writes through it.
        values[index] = unsafe { core::ptr::read_volatile(address as *const u32) };
    }
    Ok(values)
}

#[cfg(feature = "physical-wifi")]
const AX201_FIRMWARE: &[u8] = include_bytes!(env!("WOVENHAT_AX201_FIRMWARE"));

#[cfg(feature = "physical-wifi")]
const AX201_FIRMWARE_SHA256: [u8; 32] = [
    0xf8, 0x66, 0xc7, 0x8d, 0x5b, 0xc3, 0xb8, 0xf5, 0x39, 0x64, 0xbd, 0x4c, 0x2f, 0xcd, 0xfe, 0xf5,
    0xb0, 0x6e, 0xa9, 0x4f, 0x66, 0x57, 0xe0, 0x0a, 0xdc, 0x57, 0x0c, 0xa1, 0xb4, 0x90, 0x41, 0x04,
];

#[cfg(feature = "physical-wifi")]
fn ax201_firmware_is_approved() -> bool {
    let mut hasher = Sha256::new();
    hasher.update(AX201_FIRMWARE);
    let digest: [u8; 32] = hasher.finalize().into();
    digest == AX201_FIRMWARE_SHA256
}

#[cfg(feature = "physical-wifi")]
fn wait_for_ax201_command(
    console: &mut Console,
    driver: &mut crate::wifi_firmware::Ax201Driver<crate::wifi_hw::Intel22000MmioIo>,
    irq_controller: &mut crate::wifi_hw::IntelRxInterruptController,
    packet: &mut [u8],
    sequence: u16,
) -> bool {
    for _ in 0..500_000usize {
        if crate::interrupts::take_wifi_device_work() {
            if let Err(error) = irq_controller.service() {
                line(
                    console,
                    format_args!("[PHYSICAL] COMMAND_IRQ=FAILED ERROR={error:?}"),
                );
                return false;
            }
        }
        let Ok(Some(length)) = driver.poll_rx_notification(packet) else {
            core::hint::spin_loop();
            continue;
        };
        let Ok(notification) = crate::wifi_firmware::parse_intel_rx_notification(&packet[..length])
        else {
            continue;
        };
        if notification.sequence != sequence {
            continue;
        }
        return driver
            .complete_successful_command_response(&packet[..length])
            .is_ok();
    }
    false
}

#[cfg(feature = "physical-wifi")]
fn wait_for_ax201_raw_command(
    console: &mut Console,
    driver: &mut crate::wifi_firmware::Ax201Driver<crate::wifi_hw::Intel22000MmioIo>,
    irq_controller: &mut crate::wifi_hw::IntelRxInterruptController,
    packet: &mut [u8],
    sequence: u16,
) -> bool {
    for _ in 0..500_000usize {
        if crate::interrupts::take_wifi_device_work() {
            if let Err(error) = irq_controller.service() {
                line(
                    console,
                    format_args!("[PHYSICAL] COMMAND_IRQ=FAILED ERROR={error:?}"),
                );
                return false;
            }
        }
        let Ok(Some(length)) = driver.poll_rx_notification(packet) else {
            core::hint::spin_loop();
            continue;
        };
        let Ok(notification) = crate::wifi_firmware::parse_intel_rx_notification(&packet[..length])
        else {
            continue;
        };
        if notification.sequence != sequence {
            continue;
        }
        return driver.complete_command_response(&packet[..length]).is_ok();
    }
    false
}

#[cfg(feature = "physical-wifi")]
fn run_ax201_passive_scan(
    console: &mut Console,
    driver: &mut crate::wifi_firmware::Ax201Driver<crate::wifi_hw::Intel22000MmioIo>,
    irq_controller: &mut crate::wifi_hw::IntelRxInterruptController,
    packet: &mut [u8],
    device_mac: [u8; 6],
    manager: &mut Manager,
) -> bool {
    let config = crate::wifi_firmware::Ax201ScanConfig {
        enable_cam_mode: 1,
        enable_promiscuous_mode: 0,
        broadcast_station_id: 0xff,
        tx_chains: 0x0000_0003,
        rx_chains: 0x0000_0003,
    };
    let Ok(config_sequence) = driver.post_scan_config(config) else {
        line(
            console,
            format_args!("[PHYSICAL] SCAN=CONFIG_SUBMIT_FAILED"),
        );
        return false;
    };
    if !wait_for_ax201_command(console, driver, irq_controller, packet, config_sequence) {
        line(console, format_args!("[PHYSICAL] SCAN=CONFIG_REJECTED"));
        return false;
    }
    let channels = [
        crate::wifi_firmware::Ax201ScanChannel {
            number: 1,
            band: 1,
            flags: 0,
            iteration_count: 1,
            iteration_interval: 1,
        },
        crate::wifi_firmware::Ax201ScanChannel {
            number: 6,
            band: 1,
            flags: 0,
            iteration_count: 1,
            iteration_interval: 1,
        },
        crate::wifi_firmware::Ax201ScanChannel {
            number: 11,
            band: 1,
            flags: 0,
            iteration_count: 1,
            iteration_interval: 1,
        },
    ];
    const UID: u32 = 0x574f_564e;
    let Ok(request_sequence) =
        driver.post_active_scan_request_v17(UID, &channels, 20, device_mac, &[])
    else {
        line(
            console,
            format_args!("[PHYSICAL] SCAN=REQUEST_SUBMIT_FAILED"),
        );
        return false;
    };
    if !wait_for_ax201_command(console, driver, irq_controller, packet, request_sequence) {
        line(console, format_args!("[PHYSICAL] SCAN=REQUEST_REJECTED"));
        if let Ok(abort_sequence) = driver.post_scan_abort(UID) {
            let _ = wait_for_ax201_command(console, driver, irq_controller, packet, abort_sequence);
            let _ = driver.finish_scan_abort(UID);
        }
        return false;
    }
    line(
        console,
        format_args!("[PHYSICAL] SCAN=ACTIVE_BROADCAST_SUBMITTED UID={UID:08X}"),
    );
    for _ in 0..2_000_000usize {
        if crate::interrupts::take_wifi_device_work() && irq_controller.service().is_err() {
            break;
        }
        let Ok(Some(length)) = driver.poll_rx_notification(packet) else {
            core::hint::spin_loop();
            continue;
        };
        let Ok(notification) = crate::wifi_firmware::parse_intel_rx_notification(&packet[..length])
        else {
            continue;
        };
        if notification.sequence & 0x8000 == 0 {
            continue;
        }
        if notification.command == crate::wifi_firmware::AX201_RX_MPDU_CMD {
            if let Ok(rx) = crate::wifi_firmware::parse_ax201_rx_mpdu(notification) {
                if let Ok(observation) =
                    crate::wifi_scan::record_frame(manager, rx.frame, rx.signal_dbm)
                {
                    line(
                        console,
                        format_args!(
                            "[PHYSICAL] SCAN=FRAME BSSID={:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} CHANNEL={} RSSI={}",
                            observation.ap.bssid[0],
                            observation.ap.bssid[1],
                            observation.ap.bssid[2],
                            observation.ap.bssid[3],
                            observation.ap.bssid[4],
                            observation.ap.bssid[5],
                            observation.ap.channel,
                            observation.ap.signal_dbm,
                        ),
                    );
                }
            }
            continue;
        }
        match driver.handle_scan_notification(&packet[..length]) {
            Ok(crate::wifi_firmware::Ax201ScanNotification::Iteration(iteration)) => {
                line(
                    console,
                    format_args!(
                        "[PHYSICAL] SCAN=ITERATION CHANNELS={} LAST={} STATUS={}",
                        iteration.scanned_channels, iteration.last_channel, iteration.status
                    ),
                );
            }
            Ok(crate::wifi_firmware::Ax201ScanNotification::Complete(complete)) => {
                line(
                    console,
                    format_args!(
                        "[PHYSICAL] SCAN=COMPLETE STATUS={} ITERATION={} EBS={}",
                        complete.status, complete.last_iteration, complete.ebs_status
                    ),
                );
                if complete.status == 0 {
                    let _ = manager.finish_scan();
                }
                return complete.status == 0;
            }
            Err(_) => {}
        }
    }
    line(console, format_args!("[PHYSICAL] SCAN=TIMEOUT_ABORTING"));
    if let Ok(abort_sequence) = driver.post_scan_abort(UID) {
        if wait_for_ax201_command(console, driver, irq_controller, packet, abort_sequence) {
            let _ = driver.finish_scan_abort(UID);
        }
    }
    let _ = manager.finish_scan();
    false
}

#[cfg(feature = "physical-wifi")]
fn run_ax201_management_tx_probe(
    console: &mut Console,
    driver: &mut crate::wifi_firmware::Ax201Driver<crate::wifi_hw::Intel22000MmioIo>,
    irq_controller: &mut crate::wifi_hw::IntelRxInterruptController,
    packet: &mut [u8],
    device_mac: [u8; 6],
    manager: &mut Manager,
) -> bool {
    let Some(ap) = manager.scan_result(0) else {
        line(
            console,
            format_args!("[PHYSICAL] TX_PROBE=NO_SCAN_CANDIDATE"),
        );
        return false;
    };
    let mut link = crate::wifi_link::LinkMachine::new(device_mac);
    let mut frame = [0u8; crate::wifi_hw::AX201_RX_BUFFER_BYTES as usize];
    let Ok(frame_length) = link.begin(manager, ap.bssid, &mut frame) else {
        line(
            console,
            format_args!("[PHYSICAL] TX_PROBE=AUTH_BUILD_FAILED"),
        );
        return false;
    };
    let mut tx_payload = [0u8; crate::wifi_hw::AX201_TX_PAYLOAD_BYTES as usize];
    let Ok(tx_length) = crate::wifi_firmware::encode_ax201_tx_management_v9(
        &frame[..frame_length],
        crate::wifi_firmware::Ax201TxManagementOptions {
            rate_n_flags: 0x0000_0100,
            high_priority: true,
        },
        &mut tx_payload,
    ) else {
        let _ = link.timeout(manager);
        line(console, format_args!("[PHYSICAL] TX_PROBE=ENCODE_FAILED"));
        return false;
    };
    let Ok(tx_sequence) = driver.post_tx_management_payload(&tx_payload[..tx_length]) else {
        let _ = link.timeout(manager);
        line(console, format_args!("[PHYSICAL] TX_PROBE=SUBMIT_FAILED"));
        return false;
    };
    line(
        console,
        format_args!(
            "[PHYSICAL] TX_PROBE=AUTH_SUBMITTED BSSID={:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} SEQUENCE={tx_sequence:04X}",
            ap.bssid[0], ap.bssid[1], ap.bssid[2], ap.bssid[3], ap.bssid[4], ap.bssid[5]
        ),
    );
    for _ in 0..1_000_000usize {
        if crate::interrupts::take_wifi_device_work() && irq_controller.service().is_err() {
            break;
        }
        let Ok(Some(length)) = driver.poll_rx_notification(packet) else {
            core::hint::spin_loop();
            continue;
        };
        let Ok(notification) = crate::wifi_firmware::parse_intel_rx_notification(&packet[..length])
        else {
            continue;
        };
        if notification.command != crate::wifi_firmware::INTEL_TX_CMD
            || notification.group_id != crate::wifi_firmware::INTEL_LEGACY_GROUP
            || notification.sequence != tx_sequence
        {
            continue;
        }
        let Ok(response) = driver.complete_tx_response(&packet[..length]) else {
            let _ = link.timeout(manager);
            line(
                console,
                format_args!("[PHYSICAL] TX_PROBE=RESPONSE_INVALID"),
            );
            return false;
        };
        let success = response.status & 0xff == 1;
        line(
            console,
            format_args!(
                "[PHYSICAL] TX_PROBE=COMPLETE STATUS={} QUEUE={} SEQUENCE={:04X}",
                response.status, response.queue_number, response.sequence
            ),
        );
        let _ = link.timeout(manager);
        return success;
    }
    let _ = link.timeout(manager);
    line(console, format_args!("[PHYSICAL] TX_PROBE=TIMEOUT"));
    false
}

#[cfg(feature = "physical-wifi")]
pub fn run_ax201_activation(console: &mut Console, acpi: Option<&hal::acpi::Summary>) -> ! {
    serial::init();
    console.clear();
    line(console, format_args!("WOVENHAT AX201 ACTIVATION PROBE"));
    line(
        console,
        format_args!("[PHYSICAL] MODE=AX201-FIRMWARE-AND-RX-POLL"),
    );
    let hardware = hal::init(acpi);
    let mut target = None;
    for index in 0..usize::from(hardware.pci.recorded) {
        let Some(device) = hal::pci::device(index) else {
            continue;
        };
        if device.vendor_id == 0x8086
            && device.device_id == 0xa0f0
            && device.class == 2
            && device.subclass == 0x80
        {
            target = Some(device);
            break;
        }
    }
    let Some(device) = target else {
        line(console, format_args!("[PHYSICAL] AX201=NOT_FOUND"));
        line(console, format_args!("[PHYSICAL] COMPLETE=NO_TARGET"));
        physical_halt();
    };
    if !ax201_firmware_is_approved() {
        line(
            console,
            format_args!("[PHYSICAL] FIRMWARE=REJECTED HASH_MISMATCH"),
        );
        line(console, format_args!("[PHYSICAL] COMPLETE=NO_FIRMWARE"));
        physical_halt();
    }
    line(console, format_args!("[PHYSICAL] FIRMWARE=APPROVED API=77"));
    let original_command = device.command;
    line(
        console,
        format_args!(
            "[PHYSICAL] TARGET=8086:A0F0 PCI={:02X}:{:02X}.{} BAR={:016X} CMD={:04X}",
            device.bus, device.device, device.function, device.bars[0].address, original_command
        ),
    );
    let function = match crate::wifi_hw::bind_supported(device) {
        Ok(function) => function,
        Err(error) => {
            line(
                console,
                format_args!("[PHYSICAL] BIND=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    if !hal::pci::enable_memory_bus_master(function.pci.address) {
        line(
            console,
            format_args!("[PHYSICAL] PCI_DMA=FAILED COMMAND_ENABLE"),
        );
        physical_halt();
    }
    line(
        console,
        format_args!("[PHYSICAL] PCI_DMA=MEMORY_BUS_MASTER_ENABLED"),
    );
    let reset_registers = match crate::wifi_hw::map_mmio_page(function.pci.mmio_base) {
        Ok(registers) => registers,
        Err(error) => {
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] RESET_MMIO=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    let reset_csr = match crate::wifi_hw::IntelCsrBank::from_supported(function, reset_registers) {
        Ok(csr) => csr,
        Err(error) => {
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] RESET_CSR=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    let hardware_revision = match reset_csr.read(crate::wifi_hw::IntelCsr::HwRev) {
        Ok(value) if value != 0 && value != u32::MAX => match u16::try_from(value) {
            Ok(value) => value,
            Err(_) => {
                let _ = hal::pci::restore_command(function.pci.address, original_command);
                line(
                    console,
                    format_args!("[PHYSICAL] HW_REV=INVALID VALUE={value:08X}"),
                );
                physical_halt();
            }
        },
        Ok(value) => {
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] HW_REV=INVALID VALUE={value:08X}"),
            );
            physical_halt();
        }
        Err(error) => {
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] HW_REV=READ_FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    line(
        console,
        format_args!("[PHYSICAL] HW_REV={hardware_revision:04X}"),
    );
    let device_mac = match reset_csr.read_mac_address() {
        Ok(mac) => {
            line(
                console,
                format_args!(
                    "[PHYSICAL] MAC_CSR={:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                ),
            );
            mac
        }
        Err(error) => {
            line(
                console,
                format_args!("[PHYSICAL] MAC_CSR=READ_FAILED ERROR={error:?}"),
            );
            [0; 6]
        }
    };
    let mut reset = crate::wifi_hw::IntelResetController::new(reset_csr);
    if reset
        .request_reset()
        .and_then(|_| reset.begin_ready_wait())
        .and_then(|_| reset.poll_ready_bounded(crate::wifi_hw::INTEL_RESET_POLL_LIMIT))
        .is_err()
    {
        let _ = hal::pci::restore_command(function.pci.address, original_command);
        line(
            console,
            format_args!("[PHYSICAL] RESET=FAILED STATE={:?}", reset.state()),
        );
        physical_halt();
    }
    let mac_registers = match crate::wifi_hw::map_mmio_page(function.pci.mmio_base) {
        Ok(registers) => registers,
        Err(error) => {
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] MAC_MMIO=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    let mac_csr = match crate::wifi_hw::IntelCsrBank::from_supported(function, mac_registers) {
        Ok(csr) => csr,
        Err(error) => {
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] MAC_CSR=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    let mut mac = crate::wifi_hw::IntelMacAccessController::new(mac_csr);
    if mac
        .mark_init_done()
        .and_then(|_| mac.request_access())
        .and_then(|_| mac.poll_access_bounded(crate::wifi_hw::INTEL_MAC_ACCESS_POLL_LIMIT))
        .is_err()
    {
        let _ = hal::pci::restore_command(function.pci.address, original_command);
        line(
            console,
            format_args!("[PHYSICAL] MAC_ACCESS=FAILED STATE={:?}", mac.state()),
        );
        physical_halt();
    }
    line(
        console,
        format_args!("[PHYSICAL] RESET=READY MAC_ACCESS=GRANTED"),
    );
    let registers = match crate::wifi_hw::map_mmio_page(function.pci.mmio_base) {
        Ok(registers) => registers,
        Err(error) => {
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] MMIO=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    let io = match crate::wifi_hw::Intel22000MmioIo::from_supported(function, registers) {
        Ok(io) => io,
        Err(error) => {
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] MMIO_ADAPTER=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    let source = crate::wifi_firmware::Ax201FirmwareSource {
        filename: "iwlwifi-QuZ-a0-hr-b0-77.ucode",
        bytes: AX201_FIRMWARE,
    };
    let mut driver = match crate::wifi_firmware::Ax201Driver::new_from_source(function, source, io)
    {
        Ok(driver) => driver,
        Err(error) => {
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] FIRMWARE=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    if let Err(error) = driver.allocate_and_attach_queues() {
        let _ = hal::pci::restore_command(function.pci.address, original_command);
        line(
            console,
            format_args!("[PHYSICAL] QUEUES=FAILED ERROR={error:?}"),
        );
        physical_halt();
    }
    line(
        console,
        format_args!("[PHYSICAL] DMA=OWNED FW_BYTES={}", AX201_FIRMWARE.len()),
    );
    match crate::wifi_hw::bind_ax201_msi(device) {
        Ok(binding) => line(
            console,
            format_args!(
                "[PHYSICAL] MSI=PROGRAMMED VECTOR={:02X} APIC={:02X}",
                binding.vector, binding.destination_apic_id
            ),
        ),
        Err(error) => {
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] MSI=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    }
    let irq_registers = match crate::wifi_hw::map_mmio_page(function.pci.mmio_base) {
        Ok(registers) => registers,
        Err(error) => {
            let _ = hal::pci::disable_msi(device);
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] IRQ_MMIO=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    let irq_csr = match crate::wifi_hw::IntelCsrBank::from_supported(function, irq_registers) {
        Ok(csr) => csr,
        Err(error) => {
            let _ = hal::pci::disable_msi(device);
            let _ = hal::pci::restore_command(function.pci.address, original_command);
            line(
                console,
                format_args!("[PHYSICAL] IRQ_CSR=FAILED ERROR={error:?}"),
            );
            physical_halt();
        }
    };
    let mut irq_controller = crate::wifi_hw::IntelRxInterruptController::new(irq_csr);
    if let Err(error) = irq_controller.enable_rx() {
        let _ = hal::pci::disable_msi(device);
        let _ = hal::pci::restore_command(function.pci.address, original_command);
        line(
            console,
            format_args!("[PHYSICAL] IRQ_ENABLE=FAILED ERROR={error:?}"),
        );
        physical_halt();
    }
    line(
        console,
        format_args!(
            "[PHYSICAL] IRQ=CSR_RX_ENABLED VECTOR={:02X}",
            crate::interrupts::WIFI_DEVICE_VECTOR
        ),
    );
    if let Err(error) = driver.start(hardware_revision) {
        let _ = irq_controller.disable();
        let _ = hal::pci::disable_msi(device);
        let _ = hal::pci::restore_command(function.pci.address, original_command);
        line(
            console,
            format_args!("[PHYSICAL] CPU_INIT_RUN=FAILED ERROR={error:?}"),
        );
        line(console, format_args!("[PHYSICAL] POWER_OFF_REQUIRED=1"));
        physical_halt();
    }
    line(console, format_args!("[PHYSICAL] CPU_INIT_RUN=ISSUED"));

    let mut packet = [0u8; crate::wifi_hw::AX201_RX_BUFFER_BYTES as usize];
    let mut alive = false;
    let irq_count_before = crate::interrupts::wifi_device_irq_count();
    for _ in 0..5_000_000usize {
        if crate::interrupts::take_wifi_device_work() {
            if let Err(error) = irq_controller.service() {
                line(
                    console,
                    format_args!("[PHYSICAL] IRQ_SERVICE=FAILED ERROR={error:?}"),
                );
                break;
            }
        }
        let Ok(Some(length)) = driver.poll_rx_notification(&mut packet) else {
            core::hint::spin_loop();
            continue;
        };
        for abi in [
            crate::wifi_firmware::IntelAliveAbiVersion::V8,
            crate::wifi_firmware::IntelAliveAbiVersion::V7,
            crate::wifi_firmware::IntelAliveAbiVersion::V3,
        ] {
            if driver
                .dispatch_rx_notification(abi, &packet[..length])
                .is_ok()
            {
                alive = true;
                break;
            }
        }
        if alive {
            break;
        }
        core::hint::spin_loop();
    }
    if alive {
        line(
            console,
            format_args!(
                "[PHYSICAL] DEVICE_ALIVE=OK MSI_IRQS={}",
                crate::interrupts::wifi_device_irq_count().saturating_sub(irq_count_before)
            ),
        );
        let dqa_enabled = driver
            .post_dqa_enable()
            .ok()
            .map(|sequence| {
                line(
                    console,
                    format_args!("[PHYSICAL] DQA=ENABLE_SUBMITTED SEQUENCE={sequence:04X}"),
                );
                wait_for_ax201_command(
                    console,
                    &mut driver,
                    &mut irq_controller,
                    &mut packet,
                    sequence,
                )
            })
            .unwrap_or(false);
        let tx_queue_ready = dqa_enabled
            && driver
                .post_tx_queue_config(0, 0)
                .ok()
                .map(|sequence| {
                    line(
                        console,
                        format_args!(
                            "[PHYSICAL] TX_QUEUE=CONFIG_SUBMITTED SEQUENCE={sequence:04X}"
                        ),
                    );
                    if !wait_for_ax201_raw_command(
                        console,
                        &mut driver,
                        &mut irq_controller,
                        &mut packet,
                        sequence,
                    ) {
                        return false;
                    }
                    match driver.accept_tx_queue_config_response(&packet) {
                        Ok(response) => {
                            line(
                                console,
                                format_args!(
                                    "[PHYSICAL] TX_QUEUE=READY QUEUE={} WRITE_POINTER={}",
                                    response.queue_number, response.write_pointer
                                ),
                            );
                            true
                        }
                        Err(error) => {
                            line(
                                console,
                                format_args!(
                                    "[PHYSICAL] TX_QUEUE=RESPONSE_INVALID ERROR={error:?}"
                                ),
                            );
                            false
                        }
                    }
                })
                .unwrap_or(false);
        line(
            console,
            format_args!(
                "[PHYSICAL] TX_QUEUE_READY={} DQA_ENABLED={}",
                tx_queue_ready as u8, dqa_enabled as u8
            ),
        );
        let valid_device_mac = device_mac != [0; 6] && device_mac != [0xff; 6];
        let mut contexts_ready = false;
        if valid_device_mac && tx_queue_ready {
            let phy = crate::wifi_firmware::Ax201PhyContextV1 {
                id_and_color: 0,
                action: 1,
                apply_time: 0,
                tx_param_color: 0,
                channel: 1,
                band: 1,
                width: 0,
                ctrl_pos: 0,
                tx_chain_info: 0x0000_0003,
                rx_chain_info: 0x0000_0003,
                acquisition_data: 0,
                dsp_cfg_flags: 0,
            };
            let mac = crate::wifi_firmware::Ax201MacContextStaV1 {
                id_and_color: 0,
                action: 1,
                mac_type: crate::wifi_firmware::Ax201MacContextStaV1::BSS_STA_MAC_TYPE,
                tsf_id: 0,
                node_addr: device_mac,
                bssid_addr: [0; 6],
                cck_rates: 0,
                ofdm_rates: 0,
                protection_flags: 0,
                cck_short_preamble: 0x20,
                short_slot: 0x10,
                filter_flags: 1 << 6,
                qos_flags: 0,
                ac: [crate::wifi_firmware::Ax201MacQos {
                    cw_min: 3,
                    cw_max: 7,
                    aifsn: 2,
                    fifos_mask: 0,
                    edca_txop: 0,
                }; 5],
                is_assoc: 0,
                dtim_time: 0,
                dtim_tsf: 0,
                beacon_interval: 0,
                reserved: 0,
                dtim_interval: 0,
                data_policy: 0,
                listen_interval: 0,
                assoc_id: 0,
                assoc_beacon_arrive_time: 0,
                p2p_ctwin: 0,
            };
            let binding = crate::wifi_firmware::Ax201BindingContextV2 {
                id_and_color: 0,
                action: 1,
                macs: [0, 0, 0],
                phy: 0,
                lmac_id: 0,
            };
            let phy_ok = driver
                .post_phy_context_v1(phy)
                .ok()
                .map(|sequence| {
                    line(
                        console,
                        format_args!("[PHYSICAL] CONTEXT=PHY SUBMITTED SEQUENCE={sequence:04X}"),
                    );
                    wait_for_ax201_command(
                        console,
                        &mut driver,
                        &mut irq_controller,
                        &mut packet,
                        sequence,
                    )
                })
                .unwrap_or(false);
            let mac_ok = phy_ok
                && driver
                    .post_mac_context_sta_v1(mac)
                    .ok()
                    .map(|sequence| {
                        line(
                            console,
                            format_args!(
                                "[PHYSICAL] CONTEXT=MAC SUBMITTED SEQUENCE={sequence:04X}"
                            ),
                        );
                        wait_for_ax201_command(
                            console,
                            &mut driver,
                            &mut irq_controller,
                            &mut packet,
                            sequence,
                        )
                    })
                    .unwrap_or(false);
            contexts_ready = mac_ok
                && driver
                    .post_binding_context_v2(binding)
                    .ok()
                    .map(|sequence| {
                        line(
                            console,
                            format_args!(
                                "[PHYSICAL] CONTEXT=BINDING SUBMITTED SEQUENCE={sequence:04X}"
                            ),
                        );
                        wait_for_ax201_command(
                            console,
                            &mut driver,
                            &mut irq_controller,
                            &mut packet,
                            sequence,
                        )
                    })
                    .unwrap_or(false);
        }
        line(
            console,
            format_args!("[PHYSICAL] CONTEXTS_READY={}", contexts_ready as u8),
        );
        if contexts_ready {
            let mut wifi_manager = Manager::new();
            wifi_manager.power_on();
            let scan_started = wifi_manager.begin_scan().is_ok();
            let scan_ok = run_ax201_passive_scan(
                console,
                &mut driver,
                &mut irq_controller,
                &mut packet,
                device_mac,
                &mut wifi_manager,
            );
            line(
                console,
                format_args!(
                    "[PHYSICAL] SCAN_RESULT={} SCAN_STARTED={} RESULTS={}",
                    scan_ok as u8,
                    scan_started as u8,
                    wifi_manager.scan_count()
                ),
            );
            if scan_ok && wifi_manager.scan_count() != 0 {
                let tx_probe_ok = run_ax201_management_tx_probe(
                    console,
                    &mut driver,
                    &mut irq_controller,
                    &mut packet,
                    device_mac,
                    &mut wifi_manager,
                );
                line(
                    console,
                    format_args!("[PHYSICAL] TX_PROBE_RESULT={}", tx_probe_ok as u8),
                );
            }
        }
        let statistics_payload =
            crate::wifi_firmware::INTEL_STATISTICS_FLG_DISABLE_NOTIF.to_le_bytes();
        let command_sequence = match driver.post_command(
            crate::wifi_firmware::INTEL_STATISTICS_CMD,
            0,
            None,
            &statistics_payload,
        ) {
            Ok(sequence) => {
                line(
                    console,
                    format_args!("[PHYSICAL] COMMAND=STATISTICS SUBMITTED SEQUENCE={sequence:04X}"),
                );
                Some(sequence)
            }
            Err(error) => {
                line(
                    console,
                    format_args!("[PHYSICAL] COMMAND=STATISTICS SUBMIT_FAILED ERROR={error:?}"),
                );
                None
            }
        };
        let mut command_complete = false;
        if let Some(sequence) = command_sequence {
            for _ in 0..500_000usize {
                if crate::interrupts::take_wifi_device_work() {
                    let _ = irq_controller.service();
                }
                let Ok(Some(length)) = driver.poll_rx_notification(&mut packet) else {
                    core::hint::spin_loop();
                    continue;
                };
                let Ok(notification) =
                    crate::wifi_firmware::parse_intel_rx_notification(&packet[..length])
                else {
                    continue;
                };
                if notification.sequence == sequence
                    && driver.complete_command_response(&packet[..length]).is_ok()
                {
                    line(
                        console,
                        format_args!(
                            "[PHYSICAL] COMMAND=STATISTICS COMPLETE OPCODE={:02X}",
                            notification.command
                        ),
                    );
                    command_complete = true;
                    break;
                }
            }
        }
        if !command_complete {
            line(
                console,
                format_args!("[PHYSICAL] COMMAND=STATISTICS TIMEOUT"),
            );
        }
        line(console, format_args!("[PHYSICAL] RADIO=NOT_YET_QUALIFIED"));
        line(console, format_args!("[PHYSICAL] POWER_OFF_REQUIRED=1"));
    } else {
        line(console, format_args!("[PHYSICAL] DEVICE_ALIVE=TIMEOUT"));
        line(console, format_args!("[PHYSICAL] POWER_OFF_REQUIRED=1"));
    }
    let _ = irq_controller.disable();
    let _ = hal::pci::disable_msi(device);
    let _ = hal::pci::restore_command(function.pci.address, original_command);
    physical_halt()
}

#[cfg(feature = "physical-wifi")]
fn physical_halt() -> ! {
    x86_64::instructions::interrupts::disable();
    loop {
        x86_64::instructions::hlt();
    }
}

#[cfg(not(feature = "physical-wifi"))]
pub fn run(console: &mut Console, acpi: Option<&hal::acpi::Summary>) -> ! {
    serial::init();
    console.clear();
    line(
        console,
        format_args!("WOVENHAT PHYSICAL INVENTORY - NOT A RADIO TEST"),
    );
    line(
        console,
        format_args!("[PHYSICAL] MODE=READ-ONLY PCI INVENTORY"),
    );
    let hardware = hal::init(acpi);
    let vendor = match hardware.cpu_vendor {
        hal::CpuVendor::Intel => "INTEL",
        hal::CpuVendor::Amd => "AMD",
        hal::CpuVendor::Unknown => "UNKNOWN",
    };
    line(
        console,
        format_args!(
            "CPU VENDOR={} LOGICAL={} TSC={} RDRAND={} AES_NI={} AVX={} PAE={} SSE4.2={}",
            vendor,
            hardware.logical_cpus,
            hardware.cpu_features.has_tsc as u8,
            hardware.cpu_features.has_rdrand as u8,
            hardware.cpu_features.has_aes_ni as u8,
            hardware.cpu_features.has_avx as u8,
            hardware.cpu_features.has_pae as u8,
            hardware.cpu_features.has_sse4_2 as u8,
        ),
    );
    line(
        console,
        format_args!(
            "ACPI={} PCI={} RECORDED={} STORAGE={} NETWORK={} DISPLAY={} BRIDGES={} ECAM={} TRUNCATED={}",
            acpi.is_some() as u8,
            hardware.pci.discovered,
            hardware.pci.recorded,
            hardware.pci.storage,
            hardware.pci.network,
            hardware.pci.display,
            hardware.pci.bridges,
            hardware.pci.ecam as u8,
            hardware.pci.truncated as u8
        ),
    );
    let mut targets = 0;
    for index in 0..usize::from(hardware.pci.recorded) {
        let Some(device) = hal::pci::device(index) else {
            continue;
        };
        line(
            console,
            format_args!(
                "PCI {:04X}:{:02X}:{:02X}.{} {:04X}:{:04X} CLASS={:02X}:{:02X}:{:02X} REV={:02X} CMD={:04X}",
                device.segment,
                device.bus,
                device.device,
                device.function,
                device.vendor_id,
                device.device_id,
                device.class,
                device.subclass,
                device.prog_if,
                device.revision,
                device.command
            ),
        );
        if device.class != 2 {
            continue;
        }
        line(
            console,
            format_args!(
                "NET {:04X}:{:02X}:{:02X}.{} {:04X}:{:04X}",
                device.segment,
                device.bus,
                device.device,
                device.function,
                device.vendor_id,
                device.device_id
            ),
        );
        if device.vendor_id == 0x8086 && device.device_id == 0xa0f0 && device.subclass == 0x80 {
            targets += 1;
            line(
                console,
                format_args!("[PHYSICAL] TARGET=AX201 8086:A0F0 DETECTED"),
            );
            line(
                console,
                format_args!(
                    "COMMAND={:04X} MSI={} MSIX={} PCIE={} BAD_CAPS={}",
                    device.command,
                    device.capabilities.msi as u8,
                    device.capabilities.msix as u8,
                    device.capabilities.pcie as u8,
                    device.capabilities.malformed as u8
                ),
            );
            for (bar_index, bar) in device.bars.iter().enumerate().filter(|(_, bar)| bar.valid) {
                line(
                    console,
                    format_args!(
                        "BAR{}={:016X} IO={}",
                        bar_index,
                        bar.address,
                        matches!(bar.kind, hal::pci::BarKind::Io) as u8
                    ),
                );
                match read_ax201_csrs(&device) {
                Ok(csrs) => line(
                    console,
                    format_args!(
                        "[PHYSICAL] AX201_CSR_READONLY=OK HWIF={:08X} INT={:08X} INT_MASK={:08X} RESET={:08X} GP={:08X} HW_REV={:08X} GP_DRIVER={:08X}",
                        csrs[0], csrs[1], csrs[2], csrs[3], csrs[4], csrs[5], csrs[6]
                    ),
                ),
                Err(reason) => line(
                    console,
                    format_args!("[PHYSICAL] AX201_CSR_READONLY=SKIPPED REASON={reason}"),
                ),
            }
            }
        }
    }
    match crate::xhci::read_only_diagnostic() {
        Ok(diagnostic) => {
            line(
                console,
                format_args!(
                    "[PHYSICAL] XHCI BAR={:016X} CAPLEN={} SLOTS={} PORTS={} CTX64={} CMD={:08X} STATUS={:08X}",
                    diagnostic.bar,
                    diagnostic.cap_length,
                    diagnostic.max_slots,
                    diagnostic.max_ports,
                    diagnostic.context_64 as u8,
                    diagnostic.command,
                    diagnostic.status,
                ),
            );
            line(
                console,
                format_args!(
                    "XHCI DBOFF={:08X} RTSOFF={:08X} CONFIG={:08X} CONNECTED={} ENABLED={}",
                    diagnostic.doorbell_offset,
                    diagnostic.runtime_offset,
                    diagnostic.config,
                    diagnostic.connected_ports,
                    diagnostic.enabled_ports,
                ),
            );
            for port in 0..usize::from(diagnostic.port_count) {
                line(
                    console,
                    format_args!(
                        "XHCI PORT{}={:08X} CCS={} PED={} RESET={} SPEED={}",
                        port + 1,
                        diagnostic.port_status[port],
                        diagnostic.port_status[port] & 1,
                        (diagnostic.port_status[port] >> 1) & 1,
                        (diagnostic.port_status[port] >> 4) & 1,
                        (diagnostic.port_status[port] >> 10) & 0x0F,
                    ),
                );
            }
        }
        Err(error) => line(
            console,
            format_args!("[PHYSICAL] XHCI READ-ONLY DIAGNOSTIC ERROR={error:?}"),
        ),
    }
    line(console, format_args!("[PHYSICAL] AX201_COUNT={targets}"));
    line(
        console,
        format_args!("[PHYSICAL] RADIO=UNIMPLEMENTED DMA=NOT-ACTIVATED"),
    );
    line(
        console,
        format_args!("[PHYSICAL] COMPLETE - INVENTORY ONLY"),
    );
    line(
        console,
        format_args!("RECORD THIS SCREEN. POWER OFF TO LEAVE."),
    );
    serial::write_line(format_args!("[PHYSICAL] SCREEN READY"));
    crate::halt()
}
