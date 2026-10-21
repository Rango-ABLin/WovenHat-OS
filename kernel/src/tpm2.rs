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
#[allow(dead_code)]
const TPM_ST_SESSIONS: u16 = 0x0201;
const TPM_CC_GET_RANDOM: u32 = 0x0000_017b;
const TPM_CC_PCR_READ: u32 = 0x0000_017e;
#[allow(dead_code)]
const TPM_CC_NV_READ_PUBLIC: u32 = 0x0000_0169;
#[allow(dead_code)]
const TPM_CC_START_AUTH_SESSION: u32 = 0x0000_0176;
#[allow(dead_code)]
const TPM_CC_POLICY_PCR: u32 = 0x0000_017f;
#[allow(dead_code)]
const TPM_CC_NV_WRITE: u32 = 0x0000_0137;
#[allow(dead_code)]
const TPM_CC_NV_READ: u32 = 0x0000_014e;
#[allow(dead_code)]
const TPM_CC_NV_DEFINE_SPACE: u32 = 0x0000_012a;
const TPM_ALG_SHA256: u16 = 0x000b;
#[allow(dead_code)]
const TPM_ALG_RSASSA: u16 = 0x0014;
#[allow(dead_code)]
const TPM_ALG_RSAPSS: u16 = 0x0016;
#[allow(dead_code)]
const TPM_ALG_ECDSA: u16 = 0x0018;
#[allow(dead_code)]
const TPM_ST_ATTEST_QUOTE: u16 = 0x8018;
#[allow(dead_code)]
const TPM_GENERATED_VALUE: u32 = 0xff54_4347;
#[allow(dead_code)]
const TPM_ALG_NULL: u16 = 0x0010;
#[allow(dead_code)]
const TPM_SESS_POLICY: u8 = 0x01;
#[allow(dead_code)]
const TPM_RH_NULL: u32 = 0x4000_0007;
#[allow(dead_code)]
const TPM_RH_OWNER: u32 = 0x4000_0001;
#[allow(dead_code)]
const TPM_RS_PW: u32 = 0x4000_0009;
#[allow(dead_code)]
const TPMA_SESSION_CONTINUE_SESSION: u8 = 1;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicySession {
    pub handle: u32,
    pub nonce_tpm_length: u8,
    pub nonce_tpm: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyCommandResponse {
    pub nonce_tpm_length: u8,
    pub nonce_tpm: [u8; 32],
    pub attributes: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteAttestation {
    pub nonce_length: u8,
    pub nonce: [u8; 32],
    pub pcr: u8,
    pub pcr_digest: [u8; 32],
    pub clock: u64,
    pub firmware_version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteSignature {
    pub algorithm: u16,
    pub hash_algorithm: u16,
    pub first_length: u16,
    pub first: [u8; 512],
    pub second_length: u16,
    pub second: [u8; 72],
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

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut difference = 0_u8;
    for index in 0..32 {
        difference |= left[index] ^ right[index];
    }
    difference == 0
}

/// Validate the exact value returned by a policy-authorized NV read before a
/// caller promotes it to a credential root. Metadata and value are checked as
/// one decision; this function does not retain or expose either value.
#[allow(dead_code)]
pub fn validate_credential_root_readback(
    public: &NvPublic,
    expected_index: u32,
    expected_policy: &[u8; 32],
    expected_root: &[u8; 32],
    actual_root: &[u8; 32],
) -> Result<(), Error> {
    validate_credential_root_nv(public, expected_index, expected_policy)?;
    if !constant_time_equal(expected_root, actual_root) {
        return Err(Error::CredentialMismatch);
    }
    Ok(())
}

fn quote_take<'a>(bytes: &'a [u8], offset: &mut usize, length: usize) -> Result<&'a [u8], Error> {
    let end = offset.checked_add(length).ok_or(Error::MalformedResponse)?;
    if end > bytes.len() {
        return Err(Error::MalformedResponse);
    }
    let value = &bytes[*offset..end];
    *offset = end;
    Ok(value)
}

/// Parse a TPM2 quote attestation and bind its nonce, PCR selection, and
/// digest to caller-supplied expectations. The returned structure is evidence
/// only: the TPM signing key and quote signature must still be authenticated
/// before any credential decision.
#[allow(dead_code)]
pub fn decode_quote_attestation(
    quote: &[u8],
    expected_nonce: &[u8],
    expected_pcr: u8,
    expected_digest: &[u8; 32],
) -> Result<QuoteAttestation, Error> {
    if expected_nonce.is_empty() || expected_nonce.len() > 32 || expected_pcr >= 24 {
        return Err(Error::InvalidPolicy);
    }
    let mut offset = 0;
    let magic = u32::from_be_bytes(quote_take(quote, &mut offset, 4)?.try_into().unwrap());
    let structure = u16::from_be_bytes(quote_take(quote, &mut offset, 2)?.try_into().unwrap());
    if magic != TPM_GENERATED_VALUE || structure != TPM_ST_ATTEST_QUOTE {
        return Err(Error::MalformedResponse);
    }
    let signer_length = u16::from_be_bytes(quote_take(quote, &mut offset, 2)?.try_into().unwrap()) as usize;
    if signer_length > 64 {
        return Err(Error::MalformedResponse);
    }
    quote_take(quote, &mut offset, signer_length)?;
    let nonce_length = u16::from_be_bytes(quote_take(quote, &mut offset, 2)?.try_into().unwrap()) as usize;
    if nonce_length != expected_nonce.len()
        || quote_take(quote, &mut offset, nonce_length)? != expected_nonce
    {
        return Err(Error::PolicyMismatch);
    }
    let clock = u64::from_be_bytes(quote_take(quote, &mut offset, 8)?.try_into().unwrap());
    quote_take(quote, &mut offset, 4)?;
    quote_take(quote, &mut offset, 4)?;
    quote_take(quote, &mut offset, 1)?;
    let firmware_version = u64::from_be_bytes(quote_take(quote, &mut offset, 8)?.try_into().unwrap());
    let selection_count = u32::from_be_bytes(quote_take(quote, &mut offset, 4)?.try_into().unwrap());
    if selection_count != 1
        || u16::from_be_bytes(quote_take(quote, &mut offset, 2)?.try_into().unwrap()) != TPM_ALG_SHA256
        || quote_take(quote, &mut offset, 1)?[0] != 3
    {
        return Err(Error::MalformedResponse);
    }
    let selected = quote_take(quote, &mut offset, 3)?;
    let selected_byte = usize::from(expected_pcr / 8);
    if selected[0] != if selected_byte == 0 { 1 << (expected_pcr % 8) } else { 0 }
        || selected[1] != if selected_byte == 1 { 1 << (expected_pcr % 8) } else { 0 }
        || selected[2] != if selected_byte == 2 { 1 << (expected_pcr % 8) } else { 0 }
    {
        return Err(Error::PolicyMismatch);
    }
    let digest_length = u16::from_be_bytes(quote_take(quote, &mut offset, 2)?.try_into().unwrap()) as usize;
    if digest_length != 32 {
        return Err(Error::MalformedResponse);
    }
    let digest_slice = quote_take(quote, &mut offset, 32)?;
    let mut pcr_digest = [0_u8; 32];
    pcr_digest.copy_from_slice(digest_slice);
    if pcr_digest != *expected_digest || offset != quote.len() {
        return Err(Error::PolicyMismatch);
    }
    let mut nonce = [0_u8; 32];
    nonce[..nonce_length].copy_from_slice(expected_nonce);
    Ok(QuoteAttestation {
        nonce_length: nonce_length as u8,
        nonce,
        pcr: expected_pcr,
        pcr_digest,
        clock,
        firmware_version,
    })
}

/// Parse the bounded TPMT_SIGNATURE shape accompanying a quote. This checks
/// only supported SHA-256 scheme encodings; it deliberately does not claim
/// RSA/ECC cryptographic verification until a validated signing public key is
/// available from the physical TPM provisioning path.
#[allow(dead_code)]
pub fn decode_quote_signature(
    signature: &[u8],
) -> Result<(QuoteSignature, usize), Error> {
    if signature.len() < 4 {
        return Err(Error::MalformedResponse);
    }
    let algorithm = u16::from_be_bytes(signature[..2].try_into().unwrap());
    let hash_algorithm = u16::from_be_bytes(signature[2..4].try_into().unwrap());
    if hash_algorithm != TPM_ALG_SHA256 {
        return Err(Error::InvalidPolicy);
    }
    let mut parsed = QuoteSignature {
        algorithm,
        hash_algorithm,
        first_length: 0,
        first: [0_u8; 512],
        second_length: 0,
        second: [0_u8; 72],
    };
    let mut offset = 4;
    match algorithm {
        TPM_ALG_RSASSA | TPM_ALG_RSAPSS => {
            let length = u16::from_be_bytes(quote_take(signature, &mut offset, 2)?.try_into().unwrap()) as usize;
            if length == 0 || length > 512 {
                return Err(Error::MalformedResponse);
            }
            parsed.first[..length].copy_from_slice(quote_take(signature, &mut offset, length)?);
            parsed.first_length = length as u16;
        }
        TPM_ALG_ECDSA => {
            let r_length = u16::from_be_bytes(quote_take(signature, &mut offset, 2)?.try_into().unwrap()) as usize;
            if r_length == 0 || r_length > 72 {
                return Err(Error::MalformedResponse);
            }
            parsed.first[..r_length].copy_from_slice(quote_take(signature, &mut offset, r_length)?);
            parsed.first_length = r_length as u16;
            let s_length = u16::from_be_bytes(quote_take(signature, &mut offset, 2)?.try_into().unwrap()) as usize;
            if s_length == 0 || s_length > 72 {
                return Err(Error::MalformedResponse);
            }
            parsed.second[..s_length].copy_from_slice(quote_take(signature, &mut offset, s_length)?);
            parsed.second_length = s_length as u16;
        }
        _ => return Err(Error::UnsupportedInterface),
    }
    Ok((parsed, offset))
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
    CredentialMismatch,
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
fn encode_policy_session_start(nonce_caller: &[u8], output: &mut [u8; 95]) -> Result<usize, Error> {
    if nonce_caller.len() > 32 {
        return Err(Error::InvalidDescriptor);
    }
    let command_size = 27usize.saturating_add(nonce_caller.len());
    output.fill(0);
    output[..2].copy_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
    output[2..6].copy_from_slice(&(command_size as u32).to_be_bytes());
    output[6..10].copy_from_slice(&TPM_CC_START_AUTH_SESSION.to_be_bytes());
    output[10..14].copy_from_slice(&TPM_RH_NULL.to_be_bytes());
    output[14..18].copy_from_slice(&TPM_RH_NULL.to_be_bytes());
    output[18..20].copy_from_slice(&(nonce_caller.len() as u16).to_be_bytes());
    let nonce_end = 20 + nonce_caller.len();
    output[20..nonce_end].copy_from_slice(nonce_caller);
    output[nonce_end..nonce_end + 2].copy_from_slice(&0_u16.to_be_bytes());
    output[nonce_end + 2] = TPM_SESS_POLICY;
    output[nonce_end + 3..nonce_end + 5].copy_from_slice(&TPM_ALG_NULL.to_be_bytes());
    output[nonce_end + 5..nonce_end + 7].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
    Ok(command_size)
}

#[allow(dead_code)]
pub fn decode_policy_session_start(response: &[u8]) -> Result<PolicySession, Error> {
    if response.len() < 16 {
        return Err(Error::MalformedResponse);
    }
    let response_size = u32::from_be_bytes(response[2..6].try_into().unwrap()) as usize;
    let response_code = u32::from_be_bytes(response[6..10].try_into().unwrap());
    if response_size < 16 || response_size > response.len() {
        return Err(Error::MalformedResponse);
    }
    if response_code != TPM_RC_SUCCESS {
        return Err(Error::Device(response_code));
    }
    let handle = u32::from_be_bytes(response[10..14].try_into().unwrap());
    let nonce_length = u16::from_be_bytes(response[14..16].try_into().unwrap()) as usize;
    if handle == 0 || nonce_length > 32 || 16usize.saturating_add(nonce_length) != response_size {
        return Err(Error::MalformedResponse);
    }
    let mut nonce_tpm = [0_u8; 32];
    nonce_tpm[..nonce_length].copy_from_slice(&response[16..16 + nonce_length]);
    Ok(PolicySession {
        handle,
        nonce_tpm_length: nonce_length as u8,
        nonce_tpm,
    })
}

#[allow(dead_code)]
fn encode_policy_pcr(
    session_handle: u32,
    pcr: u8,
    expected_digest: &[u8; 32],
    output: &mut [u8; 58],
) -> Result<(), Error> {
    if session_handle == 0 || pcr >= 24 {
        return Err(Error::InvalidDescriptor);
    }
    output.fill(0);
    output[..2].copy_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
    output[2..6].copy_from_slice(&58_u32.to_be_bytes());
    output[6..10].copy_from_slice(&TPM_CC_POLICY_PCR.to_be_bytes());
    output[10..14].copy_from_slice(&session_handle.to_be_bytes());
    output[14..16].copy_from_slice(&32_u16.to_be_bytes());
    output[16..48].copy_from_slice(expected_digest);
    output[48..52].copy_from_slice(&1_u32.to_be_bytes());
    output[52..54].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
    output[54] = 3;
    output[55 + usize::from(pcr / 8)] = 1 << (pcr % 8);
    Ok(())
}

#[allow(dead_code)]
fn decode_policy_command_response(response: &[u8]) -> Result<(), Error> {
    if response.len() < 10 {
        return Err(Error::MalformedResponse);
    }
    let response_size = u32::from_be_bytes(response[2..6].try_into().unwrap()) as usize;
    let response_code = u32::from_be_bytes(response[6..10].try_into().unwrap());
    if response_size != 10 || response_size > response.len() {
        return Err(Error::MalformedResponse);
    }
    if response_code != TPM_RC_SUCCESS {
        return Err(Error::Device(response_code));
    }
    Ok(())
}

#[allow(dead_code)]
fn encode_nv_write_policy(
    session_handle: u32,
    nv_index: u32,
    data: &[u8],
    nonce_caller: &[u8],
    output: &mut [u8; 128],
) -> Result<usize, Error> {
    if session_handle == 0
        || data.is_empty()
        || data.len() > 32
        || nonce_caller.len() > 32
    {
        return Err(Error::InvalidDescriptor);
    }
    let auth_size = 9usize.saturating_add(nonce_caller.len());
    let command_size = 35usize
        .saturating_add(data.len())
        .saturating_add(nonce_caller.len());
    output.fill(0);
    output[..2].copy_from_slice(&TPM_ST_SESSIONS.to_be_bytes());
    output[2..6].copy_from_slice(&(command_size as u32).to_be_bytes());
    output[6..10].copy_from_slice(&TPM_CC_NV_WRITE.to_be_bytes());
    output[10..14].copy_from_slice(&nv_index.to_be_bytes());
    output[14..18].copy_from_slice(&nv_index.to_be_bytes());
    output[18..20].copy_from_slice(&(data.len() as u16).to_be_bytes());
    output[20..20 + data.len()].copy_from_slice(data);
    let offset = 20 + data.len();
    output[offset..offset + 2].copy_from_slice(&0_u16.to_be_bytes());
    output[offset + 2..offset + 6].copy_from_slice(&(auth_size as u32).to_be_bytes());
    let auth = offset + 6;
    output[auth..auth + 4].copy_from_slice(&session_handle.to_be_bytes());
    output[auth + 4..auth + 6].copy_from_slice(&(nonce_caller.len() as u16).to_be_bytes());
    output[auth + 6..auth + 6 + nonce_caller.len()].copy_from_slice(nonce_caller);
    let attributes = auth + 6 + nonce_caller.len();
    output[attributes] = TPMA_SESSION_CONTINUE_SESSION;
    output[attributes + 1..attributes + 3].copy_from_slice(&0_u16.to_be_bytes());
    Ok(command_size)
}

#[allow(dead_code)]
fn encode_nv_define_space_policy(
    owner_auth: &[u8],
    nv_index: u32,
    attributes: u32,
    policy: &[u8; 32],
    output: &mut [u8; 128],
) -> Result<usize, Error> {
    if owner_auth.is_empty() || owner_auth.len() > 32 {
        return Err(Error::InvalidDescriptor);
    }
    let authorization_size = 9usize.saturating_add(owner_auth.len());
    let parameters = 22usize.saturating_add(authorization_size);
    let command_size = parameters.saturating_add(2 + 48);
    output.fill(0);
    output[..2].copy_from_slice(&TPM_ST_SESSIONS.to_be_bytes());
    output[2..6].copy_from_slice(&(command_size as u32).to_be_bytes());
    output[6..10].copy_from_slice(&TPM_CC_NV_DEFINE_SPACE.to_be_bytes());
    output[10..14].copy_from_slice(&TPM_RH_OWNER.to_be_bytes());
    output[14..18].copy_from_slice(&nv_index.to_be_bytes());
    output[18..22].copy_from_slice(&(authorization_size as u32).to_be_bytes());
    output[22..26].copy_from_slice(&TPM_RS_PW.to_be_bytes());
    output[26..28].copy_from_slice(&0_u16.to_be_bytes());
    output[28] = 0;
    output[29..31].copy_from_slice(&(owner_auth.len() as u16).to_be_bytes());
    output[31..31 + owner_auth.len()].copy_from_slice(owner_auth);
    let public = parameters;
    output[public..public + 2].copy_from_slice(&0_u16.to_be_bytes());
    output[public + 2..public + 4].copy_from_slice(&46_u16.to_be_bytes());
    output[public + 4..public + 8].copy_from_slice(&nv_index.to_be_bytes());
    output[public + 8..public + 10].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
    output[public + 10..public + 14].copy_from_slice(&attributes.to_be_bytes());
    output[public + 14..public + 16].copy_from_slice(&32_u16.to_be_bytes());
    output[public + 16..public + 48].copy_from_slice(policy);
    output[public + 48..public + 50].copy_from_slice(&32_u16.to_be_bytes());
    Ok(command_size)
}

#[allow(dead_code)]
pub fn decode_nv_write_policy_response(response: &[u8]) -> Result<PolicyCommandResponse, Error> {
    if response.len() < 19 {
        return Err(Error::MalformedResponse);
    }
    if u16::from_be_bytes(response[..2].try_into().unwrap()) != TPM_ST_SESSIONS {
        return Err(Error::MalformedResponse);
    }
    let response_size = u32::from_be_bytes(response[2..6].try_into().unwrap()) as usize;
    let response_code = u32::from_be_bytes(response[6..10].try_into().unwrap());
    if response_size < 19 || response_size > response.len() {
        return Err(Error::MalformedResponse);
    }
    if response_code != TPM_RC_SUCCESS {
        return Err(Error::Device(response_code));
    }
    let parameter_size = u32::from_be_bytes(response[10..14].try_into().unwrap());
    if parameter_size != 0 {
        return Err(Error::MalformedResponse);
    }
    let nonce_length = u16::from_be_bytes(response[14..16].try_into().unwrap()) as usize;
    if nonce_length > 32 {
        return Err(Error::MalformedResponse);
    }
    let attributes_offset = 16 + nonce_length;
    if attributes_offset.saturating_add(3) > response_size {
        return Err(Error::MalformedResponse);
    }
    let hmac_length = u16::from_be_bytes(response[attributes_offset + 1..attributes_offset + 3].try_into().unwrap()) as usize;
    if hmac_length > 64
        || attributes_offset
            .saturating_add(3)
            .saturating_add(hmac_length)
            != response_size
    {
        return Err(Error::MalformedResponse);
    }
    let mut nonce_tpm = [0_u8; 32];
    nonce_tpm[..nonce_length].copy_from_slice(&response[16..attributes_offset]);
    Ok(PolicyCommandResponse {
        nonce_tpm_length: nonce_length as u8,
        nonce_tpm,
        attributes: response[attributes_offset],
    })
}

#[allow(dead_code)]
fn encode_nv_read_policy(
    session_handle: u32,
    nv_index: u32,
    nonce_caller: &[u8],
    output: &mut [u8; 96],
) -> Result<usize, Error> {
    if session_handle == 0 || nonce_caller.len() > 32 {
        return Err(Error::InvalidDescriptor);
    }
    let auth_size = 9usize.saturating_add(nonce_caller.len());
    let command_size = 35usize.saturating_add(nonce_caller.len());
    output.fill(0);
    output[..2].copy_from_slice(&TPM_ST_SESSIONS.to_be_bytes());
    output[2..6].copy_from_slice(&(command_size as u32).to_be_bytes());
    output[6..10].copy_from_slice(&TPM_CC_NV_READ.to_be_bytes());
    output[10..14].copy_from_slice(&nv_index.to_be_bytes());
    output[14..18].copy_from_slice(&nv_index.to_be_bytes());
    output[18..20].copy_from_slice(&32_u16.to_be_bytes());
    output[20..22].copy_from_slice(&0_u16.to_be_bytes());
    output[22..26].copy_from_slice(&(auth_size as u32).to_be_bytes());
    output[26..30].copy_from_slice(&session_handle.to_be_bytes());
    output[30..32].copy_from_slice(&(nonce_caller.len() as u16).to_be_bytes());
    output[32..32 + nonce_caller.len()].copy_from_slice(nonce_caller);
    let attributes = 32 + nonce_caller.len();
    output[attributes] = TPMA_SESSION_CONTINUE_SESSION;
    output[attributes + 1..attributes + 3].copy_from_slice(&0_u16.to_be_bytes());
    Ok(command_size)
}

#[allow(dead_code)]
pub fn decode_nv_read_policy_response(
    response: &[u8],
    output: &mut [u8; 32],
) -> Result<PolicyCommandResponse, Error> {
    if response.len() < 21 {
        return Err(Error::MalformedResponse);
    }
    if u16::from_be_bytes(response[..2].try_into().unwrap()) != TPM_ST_SESSIONS {
        return Err(Error::MalformedResponse);
    }
    let response_size = u32::from_be_bytes(response[2..6].try_into().unwrap()) as usize;
    let response_code = u32::from_be_bytes(response[6..10].try_into().unwrap());
    if response_size < 21 || response_size > response.len() {
        return Err(Error::MalformedResponse);
    }
    if response_code != TPM_RC_SUCCESS {
        return Err(Error::Device(response_code));
    }
    let parameter_size = u32::from_be_bytes(response[10..14].try_into().unwrap()) as usize;
    let data_length = u16::from_be_bytes(response[14..16].try_into().unwrap()) as usize;
    if data_length > 32 || parameter_size != 2usize.saturating_add(data_length) {
        return Err(Error::MalformedResponse);
    }
    let auth = 14 + parameter_size;
    if auth.saturating_add(5) > response_size {
        return Err(Error::MalformedResponse);
    }
    let nonce_length = u16::from_be_bytes(response[auth..auth + 2].try_into().unwrap()) as usize;
    if nonce_length > 32 || auth.saturating_add(3).saturating_add(nonce_length) > response_size {
        return Err(Error::MalformedResponse);
    }
    let attributes = auth + 2 + nonce_length;
    let hmac_length = u16::from_be_bytes(response[attributes + 1..attributes + 3].try_into().unwrap()) as usize;
    if hmac_length > 64
        || attributes
            .saturating_add(3)
            .saturating_add(hmac_length)
            != response_size
    {
        return Err(Error::MalformedResponse);
    }
    output[..data_length].copy_from_slice(&response[16..16 + data_length]);
    let mut nonce_tpm = [0_u8; 32];
    nonce_tpm[..nonce_length].copy_from_slice(&response[auth + 2..attributes]);
    Ok(PolicyCommandResponse {
        nonce_tpm_length: nonce_length as u8,
        nonce_tpm,
        attributes: response[attributes],
    })
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
        && {
            let mut session_command = [0_u8; 95];
            let caller_nonce = [0x33; 16];
            let Ok(session_size) = encode_policy_session_start(&caller_nonce, &mut session_command) else {
                return false;
            };
            if session_size != 43
                || u32::from_be_bytes(session_command[6..10].try_into().unwrap())
                    != TPM_CC_START_AUTH_SESSION
                || u32::from_be_bytes(session_command[10..14].try_into().unwrap()) != TPM_RH_NULL
                || session_command[38] != TPM_SESS_POLICY
                || u16::from_be_bytes(session_command[39..41].try_into().unwrap()) != TPM_ALG_NULL
                || u16::from_be_bytes(session_command[41..43].try_into().unwrap()) != TPM_ALG_SHA256
            {
                return false;
            }
            let mut session_response = [0_u8; 32];
            session_response[2..6].copy_from_slice(&32_u32.to_be_bytes());
            session_response[10..14].copy_from_slice(&0x0300_0000_u32.to_be_bytes());
            session_response[14..16].copy_from_slice(&16_u16.to_be_bytes());
            session_response[16..32].fill(0x77);
            let Ok(session) = decode_policy_session_start(&session_response) else {
                return false;
            };
            if session.handle != 0x0300_0000
                || session.nonce_tpm_length != 16
                || session.nonce_tpm[..16] != [0x77; 16]
            {
                return false;
            }
            let expected_pcr = [0x3c; 32];
            let mut policy_pcr = [0_u8; 58];
            if encode_policy_pcr(session.handle, 7, &expected_pcr, &mut policy_pcr).is_err()
                || u32::from_be_bytes(policy_pcr[6..10].try_into().unwrap()) != TPM_CC_POLICY_PCR
                || u32::from_be_bytes(policy_pcr[10..14].try_into().unwrap()) != session.handle
                || policy_pcr[55] != 0x80
            {
                return false;
            }
            let mut policy_response = [0_u8; 10];
            policy_response[2..6].copy_from_slice(&10_u32.to_be_bytes());
            if decode_policy_command_response(&policy_response).is_err() {
                return false;
            }
            policy_response[2..6].copy_from_slice(&9_u32.to_be_bytes());
            if decode_policy_command_response(&policy_response) != Err(Error::MalformedResponse) {
                return false;
            }
            let root_data = [0xa6; 32];
            let caller_nonce = [0x44; 8];
            let mut nv_write = [0_u8; 128];
            let Ok(nv_write_size) = encode_nv_write_policy(
                session.handle,
                0x0150_0000,
                &root_data,
                &caller_nonce,
                &mut nv_write,
            ) else {
                return false;
            };
            if nv_write_size != 75
                || u16::from_be_bytes(nv_write[..2].try_into().unwrap()) != TPM_ST_SESSIONS
                || u32::from_be_bytes(nv_write[6..10].try_into().unwrap()) != TPM_CC_NV_WRITE
                || u32::from_be_bytes(nv_write[10..14].try_into().unwrap()) != 0x0150_0000
                || u32::from_be_bytes(nv_write[14..18].try_into().unwrap()) != 0x0150_0000
                || u16::from_be_bytes(nv_write[18..20].try_into().unwrap()) != 32
                || nv_write[20..52] != root_data
                || u16::from_be_bytes(nv_write[52..54].try_into().unwrap()) != 0
                || u32::from_be_bytes(nv_write[54..58].try_into().unwrap()) != 17
                || u32::from_be_bytes(nv_write[58..62].try_into().unwrap()) != session.handle
                || u16::from_be_bytes(nv_write[62..64].try_into().unwrap()) != 8
                || nv_write[64..72] != caller_nonce
                || nv_write[72] != TPMA_SESSION_CONTINUE_SESSION
                || u16::from_be_bytes(nv_write[73..75].try_into().unwrap()) != 0
            {
                return false;
            }
            let mut nv_response = [0_u8; 19];
            nv_response[..2].copy_from_slice(&TPM_ST_SESSIONS.to_be_bytes());
            nv_response[2..6].copy_from_slice(&19_u32.to_be_bytes());
            nv_response[10..14].copy_from_slice(&0_u32.to_be_bytes());
            nv_response[14..16].copy_from_slice(&0_u16.to_be_bytes());
            nv_response[16] = TPMA_SESSION_CONTINUE_SESSION;
            nv_response[17..19].copy_from_slice(&0_u16.to_be_bytes());
            let Ok(auth_response) = decode_nv_write_policy_response(&nv_response) else {
                return false;
            };
            if auth_response.nonce_tpm_length != 0
                || auth_response.attributes != TPMA_SESSION_CONTINUE_SESSION
            {
                return false;
            }
            nv_response[2..6].copy_from_slice(&18_u32.to_be_bytes());
            if decode_nv_write_policy_response(&nv_response) != Err(Error::MalformedResponse) {
                return false;
            }
            let owner_auth = [0x99; 16];
            let root_policy = [0x5a; 32];
            let mut nv_define = [0_u8; 128];
            let Ok(nv_define_size) = encode_nv_define_space_policy(
                &owner_auth,
                0x0150_0000,
                0x0200_4008,
                &root_policy,
                &mut nv_define,
            ) else {
                return false;
            };
            if nv_define_size != 97
                || u16::from_be_bytes(nv_define[..2].try_into().unwrap()) != TPM_ST_SESSIONS
                || u32::from_be_bytes(nv_define[6..10].try_into().unwrap())
                    != TPM_CC_NV_DEFINE_SPACE
                || u32::from_be_bytes(nv_define[10..14].try_into().unwrap()) != TPM_RH_OWNER
                || u32::from_be_bytes(nv_define[14..18].try_into().unwrap()) != 0x0150_0000
                || u32::from_be_bytes(nv_define[18..22].try_into().unwrap()) != 25
                || u32::from_be_bytes(nv_define[22..26].try_into().unwrap()) != TPM_RS_PW
                || u16::from_be_bytes(nv_define[26..28].try_into().unwrap()) != 0
                || nv_define[28] != 0
                || u16::from_be_bytes(nv_define[29..31].try_into().unwrap()) != 16
                || nv_define[31..47] != owner_auth
                || u16::from_be_bytes(nv_define[47..49].try_into().unwrap()) != 0
                || u16::from_be_bytes(nv_define[49..51].try_into().unwrap()) != 46
                || u32::from_be_bytes(nv_define[51..55].try_into().unwrap()) != 0x0150_0000
                || u16::from_be_bytes(nv_define[55..57].try_into().unwrap()) != TPM_ALG_SHA256
                || u32::from_be_bytes(nv_define[57..61].try_into().unwrap()) != 0x0200_4008
                || u16::from_be_bytes(nv_define[61..63].try_into().unwrap()) != 32
                || nv_define[63..95] != root_policy
                || u16::from_be_bytes(nv_define[95..97].try_into().unwrap()) != 32
            {
                return false;
            }
            let read_nonce = [0x55; 8];
            let mut nv_read = [0_u8; 96];
            let Ok(nv_read_size) = encode_nv_read_policy(
                session.handle,
                0x0150_0000,
                &read_nonce,
                &mut nv_read,
            ) else {
                return false;
            };
            if nv_read_size != 43
                || u16::from_be_bytes(nv_read[..2].try_into().unwrap()) != TPM_ST_SESSIONS
                || u32::from_be_bytes(nv_read[6..10].try_into().unwrap()) != TPM_CC_NV_READ
                || u32::from_be_bytes(nv_read[10..14].try_into().unwrap()) != 0x0150_0000
                || u32::from_be_bytes(nv_read[14..18].try_into().unwrap()) != 0x0150_0000
                || u16::from_be_bytes(nv_read[18..20].try_into().unwrap()) != 32
                || u16::from_be_bytes(nv_read[20..22].try_into().unwrap()) != 0
                || u32::from_be_bytes(nv_read[22..26].try_into().unwrap()) != 17
                || u32::from_be_bytes(nv_read[26..30].try_into().unwrap()) != session.handle
                || u16::from_be_bytes(nv_read[30..32].try_into().unwrap()) != 8
                || nv_read[32..40] != read_nonce
                || nv_read[40] != TPMA_SESSION_CONTINUE_SESSION
                || u16::from_be_bytes(nv_read[41..43].try_into().unwrap()) != 0
            {
                return false;
            }
            let mut read_response = [0_u8; 53];
            read_response[..2].copy_from_slice(&TPM_ST_SESSIONS.to_be_bytes());
            read_response[2..6].copy_from_slice(&53_u32.to_be_bytes());
            read_response[10..14].copy_from_slice(&34_u32.to_be_bytes());
            read_response[14..16].copy_from_slice(&32_u16.to_be_bytes());
            read_response[16..48].fill(0xa6);
            read_response[48..50].copy_from_slice(&0_u16.to_be_bytes());
            read_response[50] = TPMA_SESSION_CONTINUE_SESSION;
            read_response[51..53].copy_from_slice(&0_u16.to_be_bytes());
            let mut read_data = [0_u8; 32];
            let Ok(read_auth) = decode_nv_read_policy_response(&read_response, &mut read_data) else {
                return false;
            };
            if read_data != [0xa6; 32]
                || read_auth.nonce_tpm_length != 0
                || read_auth.attributes != TPMA_SESSION_CONTINUE_SESSION
            {
                return false;
            }
            let expected_root = [0xa6; 32];
            if validate_credential_root_readback(
                &public,
                0x0150_0000,
                &expected_policy,
                &expected_root,
                &read_data,
            )
            .is_err()
            {
                return false;
            }
            let mut altered_root = read_data;
            altered_root[0] ^= 1;
            if validate_credential_root_readback(
                &public,
                0x0150_0000,
                &expected_policy,
                &expected_root,
                &altered_root,
            ) != Err(Error::CredentialMismatch)
            {
                return false;
            }
            let mut quote = [0_u8; 95];
            let quote_nonce = [0x33; 16];
            quote[..4].copy_from_slice(&TPM_GENERATED_VALUE.to_be_bytes());
            quote[4..6].copy_from_slice(&TPM_ST_ATTEST_QUOTE.to_be_bytes());
            quote[6..8].copy_from_slice(&0_u16.to_be_bytes());
            quote[8..10].copy_from_slice(&(quote_nonce.len() as u16).to_be_bytes());
            quote[10..26].copy_from_slice(&quote_nonce);
            quote[26..34].copy_from_slice(&123_u64.to_be_bytes());
            quote[43..51].copy_from_slice(&456_u64.to_be_bytes());
            quote[51..55].copy_from_slice(&1_u32.to_be_bytes());
            quote[55..57].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
            quote[57] = 3;
            quote[58] = 0x80;
            quote[61..63].copy_from_slice(&32_u16.to_be_bytes());
            quote[63..95].fill(0x3c);
            let expected_quote_digest = [0x3c; 32];
            let Ok(attestation) = decode_quote_attestation(
                &quote,
                &quote_nonce,
                7,
                &expected_quote_digest,
            ) else {
                return false;
            };
            if attestation.nonce_length != quote_nonce.len() as u8
                || attestation.pcr != 7
                || attestation.clock != 123
                || attestation.firmware_version != 456
            {
                return false;
            }
            quote[10] ^= 1;
            if decode_quote_attestation(&quote, &quote_nonce, 7, &expected_quote_digest)
                != Err(Error::PolicyMismatch)
            {
                return false;
            }
            let mut rsa_signature = [0_u8; 262];
            rsa_signature[..2].copy_from_slice(&TPM_ALG_RSASSA.to_be_bytes());
            rsa_signature[2..4].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
            rsa_signature[4..6].copy_from_slice(&256_u16.to_be_bytes());
            rsa_signature[6..].fill(0xa5);
            let Ok((rsa, rsa_size)) = decode_quote_signature(&rsa_signature) else {
                return false;
            };
            if rsa_size != 262
                || rsa.algorithm != TPM_ALG_RSASSA
                || rsa.hash_algorithm != TPM_ALG_SHA256
                || rsa.first_length != 256
            {
                return false;
            }
            let mut ecdsa_signature = [0_u8; 12];
            ecdsa_signature[..2].copy_from_slice(&TPM_ALG_ECDSA.to_be_bytes());
            ecdsa_signature[2..4].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
            ecdsa_signature[4..6].copy_from_slice(&2_u16.to_be_bytes());
            ecdsa_signature[6..8].fill(0x11);
            ecdsa_signature[8..10].copy_from_slice(&2_u16.to_be_bytes());
            ecdsa_signature[10..12].fill(0x22);
            let Ok((ecdsa, ecdsa_size)) = decode_quote_signature(&ecdsa_signature) else {
                return false;
            };
            if ecdsa_size != 12
                || ecdsa.algorithm != TPM_ALG_ECDSA
                || ecdsa.first_length != 2
                || ecdsa.second_length != 2
            {
                return false;
            }
            read_response[10..14].copy_from_slice(&33_u32.to_be_bytes());
            decode_nv_read_policy_response(&read_response, &mut read_data)
                == Err(Error::MalformedResponse)
        }
}
