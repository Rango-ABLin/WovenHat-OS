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
const CONTROL_CANCEL: u64 = 0x48;
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
const CONTROL_CANCEL_COMMAND: u32 = 1;
const TPM_INTERFACE_CRB: u32 = 1;
const TPM_ST_NO_SESSIONS: u16 = 0x8001;
const TPM_CC_GET_RANDOM: u32 = 0x0000_017b;
const TPM_CC_PCR_READ: u32 = 0x0000_017e;
#[allow(dead_code)]
const TPM_CC_NV_READ_PUBLIC: u32 = 0x0000_0169;
const TPM_ALG_SHA256: u16 = 0x000b;
const TPM_RC_SUCCESS: u32 = 0;
const TPMA_NV_OWNERWRITE: u32 = 1 << 1;
const TPMA_NV_AUTHWRITE: u32 = 1 << 2;
const TPMA_NV_POLICYWRITE: u32 = 1 << 3;
const TPMA_NV_POLICY_DELETE: u32 = 1 << 22;
const MAX_BUFFER_BYTES: usize = 0xf80;
const PCR_POLICY_MAGIC: [u8; 4] = *b"TPP1";
const PCR_POLICY_VERSION: u16 = 1;
const PCR_POLICY_SIGNED_BYTES: usize = 44;
pub const PCR_POLICY_BYTES: usize = PCR_POLICY_SIGNED_BYTES + 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NvPublic {
    pub index: u32,
    pub name_algorithm: u16,
    pub attributes: u32,
    pub auth_policy_length: u8,
    pub auth_policy: [u8; 32],
    pub data_size: u16,
}

/// Validate the public metadata of the fixed-size NV object reserved for the
/// credential root. This boundary proves only that an already-discovered
/// object has the intended policy and cannot be rewritten through ordinary
/// owner or object authorization; it does not create, write, or read it.
#[allow(dead_code)]
pub fn validate_credential_root_nv(
    public: &NvPublic,
    expected_index: u32,
    expected_policy: &[u8; 32],
) -> Result<(), Error> {
    if public.index != expected_index
        || public.name_algorithm != TPM_ALG_SHA256
        || public.data_size != 32
        || public.auth_policy_length != 32
        || public.auth_policy != *expected_policy
    {
        return Err(Error::PolicyMismatch);
    }
    if public.attributes & (TPMA_NV_OWNERWRITE | TPMA_NV_AUTHWRITE) != 0
        || public.attributes & TPMA_NV_POLICYWRITE == 0
        || public.attributes & TPMA_NV_POLICY_DELETE != 0
    {
        return Err(Error::InvalidPolicy);
    }
    Ok(())
}
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
    InvalidPolicy,
    PolicySignature,
    PolicyMismatch,
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

fn encode_pcr_read(pcr: u8, output: &mut [u8; 20]) -> Result<(), Error> {
    if pcr >= 24 {
        return Err(Error::InvalidDescriptor);
    }
    output.fill(0);
    output[..2].copy_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
    output[2..6].copy_from_slice(&20_u32.to_be_bytes());
    output[6..10].copy_from_slice(&TPM_CC_PCR_READ.to_be_bytes());
    output[10..14].copy_from_slice(&1_u32.to_be_bytes());
    output[14..16].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
    output[16] = 3;
    output[17 + usize::from(pcr / 8)] = 1 << (pcr % 8);
    Ok(())
}

#[allow(dead_code)]
fn encode_nv_read_public(index: u32, output: &mut [u8; 14]) {
    output[..2].copy_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
    output[2..6].copy_from_slice(&14_u32.to_be_bytes());
    output[6..10].copy_from_slice(&TPM_CC_NV_READ_PUBLIC.to_be_bytes());
    output[10..14].copy_from_slice(&index.to_be_bytes());
}

#[allow(dead_code)]
pub fn decode_nv_read_public(response: &[u8]) -> Result<NvPublic, Error> {
    if response.len() < 28 {
        return Err(Error::MalformedResponse);
    }
    let response_size = u32::from_be_bytes(response[2..6].try_into().unwrap()) as usize;
    let response_code = u32::from_be_bytes(response[6..10].try_into().unwrap());
    if response_size < 28 || response_size > response.len() {
        return Err(Error::MalformedResponse);
    }
    if response_code != TPM_RC_SUCCESS {
        return Err(Error::Device(response_code));
    }
    let public_size = u16::from_be_bytes(response[10..12].try_into().unwrap()) as usize;
    if public_size < 14 || 12usize.saturating_add(public_size) > response_size {
        return Err(Error::MalformedResponse);
    }
    let public_end = 12 + public_size;
    if public_end.saturating_add(2) > response_size {
        return Err(Error::MalformedResponse);
    }
    let body = &response[12..public_end];
    let index = u32::from_be_bytes(body[..4].try_into().unwrap());
    let name_algorithm = u16::from_be_bytes(body[4..6].try_into().unwrap());
    let attributes = u32::from_be_bytes(body[6..10].try_into().unwrap());
    let policy_length = u16::from_be_bytes(body[10..12].try_into().unwrap()) as usize;
    if policy_length > 32 || 12usize.saturating_add(policy_length).saturating_add(2) != body.len() {
        return Err(Error::MalformedResponse);
    }
    let mut auth_policy = [0_u8; 32];
    auth_policy[..policy_length].copy_from_slice(&body[12..12 + policy_length]);
    let data_size = u16::from_be_bytes(body[12 + policy_length..14 + policy_length].try_into().unwrap());
    let name_length = u16::from_be_bytes(response[public_end..public_end + 2].try_into().unwrap()) as usize;
    if public_end.saturating_add(2).saturating_add(name_length) != response_size {
        return Err(Error::MalformedResponse);
    }
    Ok(NvPublic {
        index,
        name_algorithm,
        attributes,
        auth_policy_length: policy_length as u8,
        auth_policy,
        data_size,
    })
}

fn decode_pcr_read(response: &[u8], pcr: u8, output: &mut [u8; 32]) -> Result<(), Error> {
    if pcr >= 24 || response.len() < 62 {
        return Err(Error::MalformedResponse);
    }
    let response_size = u32::from_be_bytes(response[2..6].try_into().unwrap()) as usize;
    let response_code = u32::from_be_bytes(response[6..10].try_into().unwrap());
    if response_size < 62 || response_size > response.len() {
        return Err(Error::MalformedResponse);
    }
    if response_code != TPM_RC_SUCCESS {
        return Err(Error::Device(response_code));
    }
    let selection_count = u32::from_be_bytes(response[14..18].try_into().unwrap());
    if selection_count != 1
        || u16::from_be_bytes(response[18..20].try_into().unwrap()) != TPM_ALG_SHA256
        || response[20] != 3
        || response[21 + usize::from(pcr / 8)] & (1 << (pcr % 8)) == 0
    {
        return Err(Error::MalformedResponse);
    }
    let digest_count = u32::from_be_bytes(response[24..28].try_into().unwrap());
    let digest_length = u16::from_be_bytes(response[28..30].try_into().unwrap());
    if digest_count != 1 || digest_length != 32 || 62 > response_size {
        return Err(Error::MalformedResponse);
    }
    output.copy_from_slice(&response[30..62]);
    Ok(())
}

/// Verify a signed PCR policy and compare its expected digest with a device
/// read. The caller supplies the provisioned policy-signing key; no key from
/// the TPM packet can create authority. This is a policy boundary, not yet a
/// sealed credential-root operation.
#[allow(dead_code)]
pub fn verify_signed_pcr_policy(
    envelope: &[u8],
    trusted_key: &[u8; 32],
    actual: &[u8; 32],
) -> Result<u8, Error> {
    if envelope.len() != PCR_POLICY_BYTES
        || envelope[..4] != PCR_POLICY_MAGIC
        || u16::from_be_bytes(envelope[4..6].try_into().unwrap()) != PCR_POLICY_VERSION
        || u16::from_be_bytes(envelope[6..8].try_into().unwrap())
            != PCR_POLICY_SIGNED_BYTES as u16
        || envelope[9..12] != [0; 3]
    {
        return Err(Error::InvalidPolicy);
    }
    let pcr = envelope[8];
    if pcr >= 24 {
        return Err(Error::InvalidPolicy);
    }
    let mut signature = [0_u8; 64];
    signature.copy_from_slice(&envelope[PCR_POLICY_SIGNED_BYTES..]);
    crate::signed_app::verify(&envelope[..PCR_POLICY_SIGNED_BYTES], trusted_key, &signature)
        .map_err(|_| Error::PolicySignature)?;
    if &envelope[12..44] != actual {
        return Err(Error::PolicyMismatch);
    }
    Ok(pcr)
}

/// Read one SHA-256 PCR through the bounded CRB transport. A PCR read is
/// device evidence only; callers must compare it to a provisioned policy or
/// verify a signed quote before using it for credential decisions.
pub fn read_pcr_sha256(info: Tpm2Info, pcr: u8, output: &mut [u8; 32]) -> Result<(), Error> {
    let mut command = [0_u8; 20];
    encode_pcr_read(pcr, &mut command)?;
    let mut response = [0_u8; MAX_BUFFER_BYTES];
    let length = execute(info, &command, &mut response)?;
    decode_pcr_read(&response[..length], pcr, output)
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

fn execute(info: Tpm2Info, command_bytes: &[u8], response_bytes: &mut [u8]) -> Result<usize, Error> {
    if info.control_area & 0xfff != 0
        || !matches!(info.start_method, 6 | 7)
        || command_bytes.is_empty()
        || command_bytes.len() > MAX_BUFFER_BYTES
        || response_bytes.len() < 12
        || response_bytes.len() > MAX_BUFFER_BYTES
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
        || command_bytes.len() > command_capacity
        || response_bytes.len() > response_capacity
    {
        return Err(Error::BufferUnavailable);
    }
    let command = buffer_mapping(info.control_area, read_u64(mapped, COMMAND_ADDRESS), command_bytes.len())?;
    let response = buffer_mapping(
        info.control_area,
        read_u64(mapped, RESPONSE_ADDRESS),
        response_bytes.len(),
    )?;
    copy_to_mmio(command, command_bytes);
    write_u32(mapped, CONTROL_REQUEST, CONTROL_REQUEST_COMMAND_READY);
    wait_for(mapped, CONTROL_REQUEST, 0)?;
    write_u32(mapped, CONTROL_START, CONTROL_START_COMMAND);
    if let Err(error) = wait_for(mapped, CONTROL_START, 0) {
        write_u32(mapped, CONTROL_CANCEL, CONTROL_CANCEL_COMMAND);
        let _ = wait_for(mapped, CONTROL_START, 0);
        write_u32(mapped, CONTROL_REQUEST, CONTROL_REQUEST_GO_IDLE);
        let _ = wait_for(mapped, CONTROL_REQUEST, 0);
        return Err(error);
    }
    copy_from_mmio(response, response_bytes);
    write_u32(mapped, CONTROL_REQUEST, CONTROL_REQUEST_GO_IDLE);
    wait_for(mapped, CONTROL_REQUEST, 0)?;
    if response_bytes.len() < 6 {
        return Err(Error::MalformedResponse);
    }
    let response_size = u32::from_be_bytes(response_bytes[2..6].try_into().unwrap()) as usize;
    if response_size < 10 || response_size > response_bytes.len() {
        return Err(Error::MalformedResponse);
    }
    Ok(response_size)
}

/// Ask a TPM2 CRB device for bounded hardware-generated bytes.
///
/// This operation is deliberately not used as a persistent credential root:
/// random output has no stable reboot identity and is not attestation.
pub fn get_random(info: Tpm2Info, output: &mut [u8]) -> Result<usize, Error> {
    if output.is_empty() || output.len() > 32 {
        return Err(Error::InvalidDescriptor);
    }
    let mut command_bytes = [0_u8; 12];
    encode_get_random(output.len() as u16, &mut command_bytes);
    let mut response_bytes = [0_u8; MAX_BUFFER_BYTES];
    let length = execute(info, &command_bytes, &mut response_bytes)?;
    decode_get_random(&response_bytes[..length], output)
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
    if decode_get_random(&response, &mut output) != Ok(32) || output != [0xa5; 32] {
        return false;
    }
    let mut pcr_command = [0_u8; 20];
    if encode_pcr_read(7, &mut pcr_command).is_err()
        || u32::from_be_bytes(pcr_command[6..10].try_into().unwrap()) != TPM_CC_PCR_READ
        || pcr_command[17] != 0x80
    {
        return false;
    }
    let mut pcr_response = [0_u8; 62];
    pcr_response[2..6].copy_from_slice(&62_u32.to_be_bytes());
    pcr_response[14..18].copy_from_slice(&1_u32.to_be_bytes());
    pcr_response[18..20].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
    pcr_response[20] = 3;
    pcr_response[21] = 0x80;
    pcr_response[24..28].copy_from_slice(&1_u32.to_be_bytes());
    pcr_response[28..30].copy_from_slice(&32_u16.to_be_bytes());
    pcr_response[30..62].fill(0x3c);
    let mut pcr = [0_u8; 32];
    if decode_pcr_read(&pcr_response, 7, &mut pcr).is_err() || pcr != [0x3c; 32] {
        return false;
    }
    pcr_response[20] = 2;
    if decode_pcr_read(&pcr_response, 7, &mut pcr) != Err(Error::MalformedResponse) {
        return false;
    }
    let pair = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new([0x42; 32]));
    let mut policy = [0_u8; PCR_POLICY_BYTES];
    policy[..4].copy_from_slice(&PCR_POLICY_MAGIC);
    policy[4..6].copy_from_slice(&PCR_POLICY_VERSION.to_be_bytes());
    policy[6..8].copy_from_slice(&(PCR_POLICY_SIGNED_BYTES as u16).to_be_bytes());
    policy[8] = 7;
    policy[12..44].fill(0x3c);
    let signature = pair.sk.sign(&policy[..PCR_POLICY_SIGNED_BYTES], None);
    policy[PCR_POLICY_SIGNED_BYTES..].copy_from_slice(&*signature);
    let mut public_key = [0_u8; 32];
    public_key.copy_from_slice(&*pair.pk);
    let actual = [0x3c; 32];
    if verify_signed_pcr_policy(&policy, &public_key, &actual) != Ok(7) {
        return false;
    }
    policy[12] ^= 1;
    if verify_signed_pcr_policy(&policy, &public_key, &actual) != Err(Error::PolicySignature) {
        return false;
    }
    let mut nv_command = [0_u8; 14];
    encode_nv_read_public(0x0150_0000, &mut nv_command);
    if u32::from_be_bytes(nv_command[6..10].try_into().unwrap()) != TPM_CC_NV_READ_PUBLIC {
        return false;
    }
    let mut nv_response = [0_u8; 60];
    nv_response[2..6].copy_from_slice(&60_u32.to_be_bytes());
    nv_response[10..12].copy_from_slice(&46_u16.to_be_bytes());
    nv_response[12..16].copy_from_slice(&0x0150_0000_u32.to_be_bytes());
    nv_response[16..18].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
    nv_response[18..22].copy_from_slice(&0x0200_0008_u32.to_be_bytes());
    let expected_policy = [0x5a; 32];
    nv_response[22..24].copy_from_slice(&32_u16.to_be_bytes());
    nv_response[24..56].fill(0x5a);
    nv_response[56..58].copy_from_slice(&32_u16.to_be_bytes());
    nv_response[58..60].copy_from_slice(&0_u16.to_be_bytes());
    let Ok(public) = decode_nv_read_public(&nv_response) else {
        return false;
    };
    if validate_credential_root_nv(&public, 0x0150_0000, &expected_policy).is_err() {
        return false;
    }
    let mut altered = public;
    altered.attributes |= TPMA_NV_OWNERWRITE;
    if validate_credential_root_nv(&altered, 0x0150_0000, &expected_policy)
        != Err(Error::InvalidPolicy)
    {
        return false;
    }
    altered = public;
    altered.auth_policy[0] ^= 1;
    if validate_credential_root_nv(&altered, 0x0150_0000, &expected_policy)
        != Err(Error::PolicyMismatch)
    {
        return false;
    }
    public.index == 0x0150_0000
        && public.name_algorithm == TPM_ALG_SHA256
        && public.attributes == 0x0200_0008
        && public.auth_policy_length == 32
        && public.data_size == 32
}
