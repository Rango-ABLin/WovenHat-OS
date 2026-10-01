//! Read-only PCI inventory boot mode for the selected physical machine.
//! Reports to framebuffer and serial, then halts before device activation.

use crate::{console::Console, hal, serial};
use core::fmt::{self, Write};

struct Screen<'a, 'b>(&'a mut Console<'b>);
impl Write for Screen<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        // The console font lacks square brackets. Keep serial markers exact
        // while displaying the same labels without unsupported glyphs.
        for part in text.split(['[', ']']) {
            self.0.print(part);
        }
        Ok(())
    }
}

fn line(console: &mut Console<'_>, args: fmt::Arguments<'_>) {
    serial::write_line(args);
    let _ = Screen(console).write_fmt(args);
    console.println("");
}

pub fn run(console: &mut Console<'_>, acpi: Option<&hal::acpi::Summary>) -> ! {
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
            }
        }
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
