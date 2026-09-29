pub mod acpi;
pub mod cpu;
pub mod pci;

#[derive(Clone, Copy)]
pub struct HardwareInfo {
    pub cpu_vendor: CpuVendor,
    pub cpu_features: CpuFeatures,
    pub logical_cpus: u32,
    pub pci: pci::Summary,
}

#[derive(Clone, Copy)]
pub enum CpuVendor {
    Intel,
    Amd,
    Unknown,
}

#[derive(Clone, Copy, Default)]
pub struct CpuFeatures {
    pub has_tsc: bool,
    pub has_rdrand: bool,
    pub has_aes_ni: bool,
    pub has_avx: bool,
    pub has_pae: bool,
    pub has_sse4_2: bool,
}

pub fn init(acpi: Option<&acpi::Summary>) -> HardwareInfo {
    if let Some(summary) = acpi {
        pci::configure(&summary.mcfg_allocations[..summary.mcfg_allocation_count]);
        let mut apertures = pci::HostApertures::default();
        let mut valid = true;
        for resource in &summary.pci_root_resources[..summary.pci_root_resource_count] {
            // WovenHat currently allocates CPU-visible BAR addresses directly.
            // Non-zero translation requires host-bridge translation support
            // before the resource can be admitted safely.
            if resource.length == 0
                || resource.base.checked_add(resource.length).is_none()
                || resource.translation_offset != 0
                || !matches!(resource.address_width, 16 | 32 | 64)
            {
                valid = false;
                break;
            }
            let range = pci::HostAperture { base: resource.base, size: resource.length };
            let result = match resource.kind {
                acpi::PciRootResourceKind::Io => apertures.io.push(range),
                acpi::PciRootResourceKind::Memory if resource.prefetchable => {
                    apertures.prefetch.push(range)
                }
                acpi::PciRootResourceKind::Memory => apertures.memory.push(range),
            };
            if result.is_err() {
                valid = false;
                break;
            }
        }
        if valid && summary.pci_root_resource_count != 0 {
            if let Err(error) = pci::configure_host_apertures(apertures) {
                crate::serial::write_line(format_args!(
                    "[PCI] host apertures rejected: {error:?}"
                ));
            }
        } else if summary.pci_root_resource_count != 0 {
            crate::serial::write_line(format_args!(
                "[PCI] host apertures rejected: invalid ACPI _CRS resource set"
            ));
        }
    } else {
        pci::configure(&[]);
    }
    HardwareInfo {
        cpu_vendor: cpu::detect_vendor(),
        cpu_features: cpu::detect_features(),
        logical_cpus: cpu::count_logical_cpus(),
        pci: pci::discover(),
    }
}
