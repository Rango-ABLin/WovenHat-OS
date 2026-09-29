use super::*;
use crate::irq_lock::IrqMutex;

const INTERRUPT_MODE_SLOTS: usize = super::topology::MAX_FUNCTIONS;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InterruptMode {
    #[default]
    None,
    Msi,
    Msix,
}

#[derive(Clone, Copy, Debug, Default)]
struct ModeSlot {
    function: topology::FunctionHandle,
    owner: u32,
    mode: InterruptMode,
}

static INTERRUPT_MODES: IrqMutex<[ModeSlot; INTERRUPT_MODE_SLOTS]> =
    IrqMutex::with_rank([ModeSlot { function: topology::FunctionHandle { slot: 0, generation: 0 }, owner: 0, mode: InterruptMode::None }; INTERRUPT_MODE_SLOTS], 22);

pub(super) fn claim_interrupt_mode(function: topology::FunctionHandle, owner: u32, mode: InterruptMode) -> bool {
    if owner == 0 || mode == InterruptMode::None { return false; }
    let mut slots = INTERRUPT_MODES.lock();
    if slots.iter().any(|slot| slot.mode != InterruptMode::None && slot.function == function) { return false; }
    let Some(slot) = slots.iter_mut().find(|slot| slot.mode == InterruptMode::None) else { return false; };
    *slot = ModeSlot { function, owner, mode };
    true
}

pub(super) fn release_interrupt_mode(function: topology::FunctionHandle, owner: u32, mode: InterruptMode) -> bool {
    let mut slots = INTERRUPT_MODES.lock();
    let Some(slot) = slots.iter_mut().find(|slot| slot.function == function && slot.owner == owner && slot.mode == mode) else { return false; };
    *slot = ModeSlot::default();
    true
}

static VECTOR_ALLOCATOR: IrqMutex<vector::Allocator> =
    IrqMutex::with_rank(vector::Allocator::new(), 21);

pub(super) fn allocate_vector(owner: u32) -> Result<vector::Lease, vector::Error> {
    VECTOR_ALLOCATOR.lock().allocate(owner)
}

pub(super) fn validate_vector(lease: vector::Lease, owner: u32) -> Result<(), vector::Error> {
    VECTOR_ALLOCATOR.lock().validate(lease, owner)
}

pub(super) fn release_vector(lease: vector::Lease, owner: u32) -> Result<(), vector::Error> {
    VECTOR_ALLOCATOR.lock().release(lease, owner)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MsiLease {
    pub function: topology::FunctionHandle,
    pub vector: vector::Lease,
    pub destination_apic_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsiLifecycleError {
    InvalidFunction,
    Vector(vector::Error),
    Msi(MsiError),
    /// Hardware quiesce or post-quiesce cleanup failed after activation.
    /// The caller must retain this lease and retry ordered disable; it must
    /// never make the vector available for reuse while the obligation exists.
    ActivationRetained {
        error: MsiError,
        lease: MsiLease,
    },
}

impl From<vector::Error> for MsiLifecycleError {
    fn from(error: vector::Error) -> Self { Self::Vector(error) }
}

impl From<MsiError> for MsiLifecycleError {
    fn from(error: MsiError) -> Self { Self::Msi(error) }
}

pub const PCI_CAP_ID_MSI: u8 = 0x05;
pub const PCI_MSI_ENABLE: u16 = 1 << 0;
pub const PCI_MSI_64BIT_CAPABLE: u16 = 1 << 7;
pub const PCI_MSI_PER_VECTOR_MASKING: u16 = 1 << 8;

/// x86 MSI message-address base for local APIC fixed delivery.
pub const X86_MSI_ADDRESS_BASE: u32 = 0xfee0_0000;
pub const X86_MSI_DESTINATION_SHIFT: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsiError {
    NoCapability,
    MalformedCapability,
    UnsupportedDestination,
    ConfigRead,
    ConfigWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MsiCapability {
    pub offset: u16,
    pub control: u16,
    pub is_64_bit: bool,
    pub per_vector_masking: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MsiMessage {
    pub address_low: u32,
    pub address_high: u32,
    pub data: u16,
}

impl MsiMessage {
    pub fn fixed(destination_apic_id: u32, vector: u8) -> Result<Self, MsiError> {
        if destination_apic_id > u8::MAX as u32 {
            return Err(MsiError::UnsupportedDestination);
        }
        Ok(Self {
            address_low: X86_MSI_ADDRESS_BASE | (destination_apic_id << X86_MSI_DESTINATION_SHIFT),
            address_high: 0,
            data: u16::from(vector),
        })
    }
}

pub fn msi_capability(device: Device) -> Result<MsiCapability, MsiError> {
    if !device.capabilities.msi || device.capabilities.msi_offset == 0 {
        return Err(MsiError::NoCapability);
    }
    let address = Address {
        segment: device.segment,
        bus: device.bus,
        device: device.device,
        function: device.function,
    };
    let header =
        read_config(address, device.capabilities.msi_offset).ok_or(MsiError::ConfigRead)?;
    if header as u8 != PCI_CAP_ID_MSI {
        return Err(MsiError::MalformedCapability);
    }
    let control = (header >> 16) as u16;
    Ok(MsiCapability {
        offset: device.capabilities.msi_offset,
        control,
        is_64_bit: control & PCI_MSI_64BIT_CAPABLE != 0,
        per_vector_masking: control & PCI_MSI_PER_VECTOR_MASKING != 0,
    })
}

/// Program one MSI vector while keeping the capability disabled until all
/// address/data fields are valid. WovenHat currently uses one fixed-delivery
/// vector and does not enable multiple-message MSI.
pub fn program_msi(
    device: Device,
    destination_apic_id: u32,
    vector: u8,
) -> Result<MsiCapability, MsiError> {
    let capability = msi_capability(device)?;
    let message = MsiMessage::fixed(destination_apic_id, vector)?;
    let address = Address {
        segment: device.segment,
        bus: device.bus,
        device: device.device,
        function: device.function,
    };

    let _guard = CONFIG_LOCK.lock();
    let base = capability.offset;

    let Some(header) = read_config_unlocked(address, base) else {
        return Err(MsiError::ConfigRead);
    };
    let mut control = (header >> 16) as u16;
    control &= !PCI_MSI_ENABLE;
    // Multiple-message enable bits [6:4] stay zero: one vector only.
    control &= !(0b111 << 4);
    let disabled_header = (header & 0x0000_ffff) | (u32::from(control) << 16);
    if !write_config_unlocked(address, base, disabled_header) {
        return Err(MsiError::ConfigWrite);
    }

    if !write_config_unlocked(address, base + 4, message.address_low) {
        return Err(MsiError::ConfigWrite);
    }

    let data_dword_offset = if capability.is_64_bit {
        if !write_config_unlocked(address, base + 8, message.address_high) {
            return Err(MsiError::ConfigWrite);
        }
        base + 12
    } else {
        base + 8
    };

    let Some(old_data_dword) = read_config_unlocked(address, data_dword_offset) else {
        return Err(MsiError::ConfigRead);
    };
    let new_data_dword = (old_data_dword & 0xffff_0000) | u32::from(message.data);
    if !write_config_unlocked(address, data_dword_offset, new_data_dword) {
        return Err(MsiError::ConfigWrite);
    }

    control |= PCI_MSI_ENABLE;
    let enabled_header = (header & 0x0000_ffff) | (u32::from(control) << 16);
    if !write_config_unlocked(address, base, enabled_header) {
        return Err(MsiError::ConfigWrite);
    }

    Ok(MsiCapability {
        control,
        ..capability
    })
}

pub fn disable_msi(device: Device) -> Result<(), MsiError> {
    let capability = msi_capability(device)?;
    let address = Address {
        segment: device.segment,
        bus: device.bus,
        device: device.device,
        function: device.function,
    };
    let _guard = CONFIG_LOCK.lock();
    let Some(header) = read_config_unlocked(address, capability.offset) else {
        return Err(MsiError::ConfigRead);
    };
    let control = ((header >> 16) as u16) & !PCI_MSI_ENABLE;
    let new_header = (header & 0x0000_ffff) | (u32::from(control) << 16);
    if !write_config_unlocked(address, capability.offset, new_header) {
        return Err(MsiError::ConfigWrite);
    }
    Ok(())
}

/// Pure acceptance test for x86 MSI encoding. Physical configuration-space
/// writes are intentionally deferred until a real supported PCI function is
/// explicitly bound by the Wi-Fi transport.
pub fn stage13_10aa_msi_self_test() -> bool {
    let Ok(message) = MsiMessage::fixed(0x2a, crate::interrupts::WIFI_DEVICE_VECTOR) else {
        return false;
    };
    message.address_low == 0xfee2_a000
        && message.address_high == 0
        && message.data == u16::from(crate::interrupts::WIFI_DEVICE_VECTOR)
        && MsiMessage::fixed(0x100, crate::interrupts::WIFI_DEVICE_VECTOR)
            == Err(MsiError::UnsupportedDestination)
}


/// Allocate and program one owner-bound MSI vector. If configuration-space
/// programming fails, the vector reservation is rolled back before returning.
#[expect(dead_code)]
pub fn enable_owned_msi(
    function: topology::FunctionHandle,
    owner: u32,
    destination_apic_id: u32,
) -> Result<MsiLease, MsiLifecycleError> {
    let (snapshot, device) = super::function_device(function)
        .ok_or(MsiLifecycleError::InvalidFunction)?;
    if snapshot.owner != Some(owner) || owner == 0 {
        return Err(MsiLifecycleError::InvalidFunction);
    }

    if !claim_interrupt_mode(function, owner, InterruptMode::Msi) {
        return Err(MsiLifecycleError::InvalidFunction);
    }
    let lease = match VECTOR_ALLOCATOR.lock().allocate(owner) {
        Ok(lease) => lease,
        Err(error) => {
            let _ = release_interrupt_mode(function, owner, InterruptMode::Msi);
            return Err(error.into());
        }
    };
    if let Err(error) = program_msi(device, destination_apic_id, lease.vector) {
        let _ = VECTOR_ALLOCATOR.lock().release(lease, owner);
        let _ = release_interrupt_mode(function, owner, InterruptMode::Msi);
        return Err(MsiLifecycleError::Msi(error));
    }
    Ok(MsiLease { function, vector: lease, destination_apic_id })
}

/// Disable MSI before releasing its vector. A failed disable deliberately
/// retains the vector lease so a still-live device can never target a reused
/// interrupt vector.
#[expect(dead_code)]
pub fn disable_owned_msi(lease: MsiLease, owner: u32) -> Result<(), MsiLifecycleError> {
    VECTOR_ALLOCATOR.lock().validate(lease.vector, owner)?;
    let (snapshot, device) = super::function_device(lease.function)
        .ok_or(MsiLifecycleError::InvalidFunction)?;
    if snapshot.owner != Some(owner) || owner == 0 {
        return Err(MsiLifecycleError::InvalidFunction);
    }
    if let Err(error) = disable_msi(device) {
        return Err(MsiLifecycleError::ActivationRetained { error, lease });
    }
    VECTOR_ALLOCATOR.lock().release(lease.vector, owner)?;
    if !release_interrupt_mode(lease.function, owner, InterruptMode::Msi) {
        return Err(MsiLifecycleError::InvalidFunction);
    }
    Ok(())
}
