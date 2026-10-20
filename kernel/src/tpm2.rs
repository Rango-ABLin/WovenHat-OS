//! Bounded TPM 2.0 CRB transport.
//!
//! The ACPI TPM2 descriptor is only an inventory record. This module performs
//! the next trust-boundary step: it serializes one polling CRB transaction,
//! validates the advertised buffers and the TPM response, and never treats a
//! malformed or timed-out response as credential material. It currently
//! exposes `GetRandom` as a transport probe; stable credential roots still
//! require authenticated PCR policy and sealed NV provisioning.

use crate::{hal::acpi::Tpm2Info, paging};

const INTERFACE_ID: u64 = 0x30;
const CONTROL_REQUEST: u64 = 0x40;
const CONTROL_STATUS: u64 = 0x44;
const CONTROL_START: u64 = 0x4c;
const COMMAND_SIZE: u64 = 0x58;
const COMMAND_ADDRESS: u64 = 0x5c;
const RESPONSE_SIZE: u64 = 0x64;
const RESPONSE_ADDRESS: u64 = 0x68;
const DATA_BUFFER: u64 = 0x80;
const CONTROL_REQUEST_COMMAND_READY: u32 = 1;
const CONTROL_REQUEST_GO_IDLE: u32 = 2;
const CONTROL_STATUS_FATAL: u32 = 1;
const CONTROL_START_COMMAND: u32 = 1;
const TPM_INTERFACE_CRB: u32 = 1;
const TPM_ST_NO_SESSIONS: u16 = 0x8001;
const TPM_CC_GET_RANDOM: u32 = 0x0000_017b;
const TPM_RC_SUCCESS: u32 = 0;
const MAX_BUFFER_BYTES: usize = 0xf80;
// The probe runs before timer IRQs are enabled, so this bound must not depend
// on the kernel tick counter. It is deliberately finite even for a wedged
// device or malformed firmware descriptor.
const MAX_POLL_ITERATIONS: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidDescriptor,
    MmioUnavailable,
    UnsupportedInterface,
    BufferUnavailable,
    Busy,
    Timeout,
    MalformedResponse,
    Device(u32),
}

static TRANSACTION: spin::Mutex<()> = spin::Mutex::new(());

fn read_u32(base: u64, offset: u64) -> u32 {
    // SAFETY: callers map the validated TPM CRB page as uncached MMIO before
    // accessing these fixed-width device registers.
    unsafe { ((base + offset) as *const u32).read_volatile() }
}

fn write_u32(base: u64, offset: u64, value: u32) {
    // SAFETY: callers map the validated TPM CRB page as writable uncached
    // MMIO before accessing these fixed-width device registers.
    unsafe { ((base + offset) as *mut u32).write_volatile(value) }
}

fn read_u64(base: u64, offset: u64) -> u64 {
    let low = u64::from(read_u32(base, offset));
    let high = u64::from(read_u32(base, offset + 4));
    low | (high << 32)
}

fn copy_to_mmio(base: u64, bytes: &[u8]) {
    // SAFETY: the CRB data buffer has been range-checked by the caller and is
    // mapped as writable uncached MMIO.
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), base as *mut u8, bytes.len());
    }
}

fn copy_from_mmio(base: u64, output: &mut [u8]) {
    // SAFETY: the CRB data buffer has been range-checked by the caller and is
    // mapped as readable uncached MMIO.
    unsafe {
        core::ptr::copy_nonoverlapping(base as *const u8, output.as_mut_ptr(), output.len());
    }
}

fn wait_for(base: u64, offset: u64, expected: u32) -> Result<(), Error> {
    for _ in 0..MAX_POLL_ITERATIONS {
        if read_u32(base, offset) == expected {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(Error::Timeout)
}

fn encode_get_random(request: u16, output: &mut [u8; 12]) {
    output[..2].copy_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
    output[2..6].copy_from_slice(&12_u32.to_be_bytes());
    output[6..10].copy_from_slice(&TPM_CC_GET_RANDOM.to_be_bytes());
    output[10..12].copy_from_slice(&request.to_be_bytes());
}

fn decode_get_random(response: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    if response.len() < 12 {
        return Err(Error::MalformedResponse);
    }
    let response_size = u32::from_be_bytes(response[2..6].try_into().unwrap()) as usize;
    let response_code = u32::from_be_bytes(response[6..10].try_into().unwrap());
    if response_size < 12 || response_size > response.len() {
        return Err(Error::MalformedResponse);
    }
    if response_code != TPM_RC_SUCCESS {
        return Err(Error::Device(response_code));
    }
    let length = u16::from_be_bytes(response[10..12].try_into().unwrap()) as usize;
    if length == 0 || length > output.len() || 12usize.saturating_add(length) > response_size {
        return Err(Error::MalformedResponse);
    }
    output[..length].copy_from_slice(&response[12..12 + length]);
    Ok(length)
}

fn buffer_mapping(control: u64, address: u64, length: usize) -> Result<u64, Error> {
    let address = if address == 0 {
        control.checked_add(DATA_BUFFER).ok_or(Error::BufferUnavailable)?
    } else {
        address
    };
    let page = address & !0xfff;
    let offset = usize::try_from(address & 0xfff).map_err(|_| Error::BufferUnavailable)?;
    if offset.saturating_add(length) > 0x1000 {
        return Err(Error::BufferUnavailable);
    }
    let mapped = paging::map_mmio(page).map_err(|_| Error::MmioUnavailable)?;
    mapped
        .checked_add(address & 0xfff)
        .ok_or(Error::BufferUnavailable)
}

/// Ask a TPM2 CRB device for bounded hardware-generated bytes.
///
/// This operation is deliberately not used as a persistent credential root:
/// random output has no stable reboot identity and is not attestation.
pub fn get_random(info: Tpm2Info, output: &mut [u8]) -> Result<usize, Error> {
    if info.control_area & 0xfff != 0
        || !matches!(info.start_method, 6 | 7)
        || output.is_empty()
        || output.len() > 32
    {
        return Err(Error::InvalidDescriptor);
    }
    let _transaction = TRANSACTION.lock();
    let mapped = paging::map_mmio(info.control_area).map_err(|_| Error::MmioUnavailable)?;
    if read_u32(mapped, INTERFACE_ID) & 0x0f != TPM_INTERFACE_CRB {
        return Err(Error::UnsupportedInterface);
    }
    if read_u32(mapped, CONTROL_STATUS) & CONTROL_STATUS_FATAL != 0 {
        return Err(Error::Busy);
    }
    let command_capacity = read_u32(mapped, COMMAND_SIZE) as usize;
    let response_capacity = read_u32(mapped, RESPONSE_SIZE) as usize;
    if !(12..=MAX_BUFFER_BYTES).contains(&command_capacity)
        || !(12..=MAX_BUFFER_BYTES).contains(&response_capacity)
    {
        return Err(Error::BufferUnavailable);
    }
    let command = buffer_mapping(info.control_area, read_u64(mapped, COMMAND_ADDRESS), 12)?;
    let response = buffer_mapping(
        info.control_area,
        read_u64(mapped, RESPONSE_ADDRESS),
        response_capacity.min(MAX_BUFFER_BYTES),
    )?;
    let mut command_bytes = [0_u8; 12];
    encode_get_random(output.len() as u16, &mut command_bytes);
    copy_to_mmio(command, &command_bytes);
    write_u32(mapped, CONTROL_REQUEST, CONTROL_REQUEST_COMMAND_READY);
    wait_for(mapped, CONTROL_REQUEST, 0)?;
    write_u32(mapped, CONTROL_START, CONTROL_START_COMMAND);
    wait_for(mapped, CONTROL_START, 0)?;
    let mut response_bytes = [0_u8; MAX_BUFFER_BYTES];
    copy_from_mmio(response, &mut response_bytes[..response_capacity]);
    write_u32(mapped, CONTROL_REQUEST, CONTROL_REQUEST_GO_IDLE);
    wait_for(mapped, CONTROL_REQUEST, 0)?;
    decode_get_random(&response_bytes[..response_capacity], output)
}

#[cfg(feature = "qemu-test")]
pub fn self_test() -> bool {
    let mut command = [0_u8; 12];
    encode_get_random(32, &mut command);
    if u16::from_be_bytes(command[..2].try_into().unwrap()) != TPM_ST_NO_SESSIONS
        || u32::from_be_bytes(command[6..10].try_into().unwrap()) != TPM_CC_GET_RANDOM
    {
        return false;
    }
    let mut response = [0_u8; 44];
    response[2..6].copy_from_slice(&44_u32.to_be_bytes());
    response[10..12].copy_from_slice(&32_u16.to_be_bytes());
    response[12..44].fill(0xa5);
    let mut output = [0_u8; 32];
    decode_get_random(&response, &mut output) == Ok(32) && output == [0xa5; 32]
}
