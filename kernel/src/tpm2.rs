//! Bounded TPM 2.0 CRB transport.
//!
//! The ACPI TPM2 descriptor is only an inventory record. This module performs
//! the next trust-boundary step: it serializes one polling CRB transaction,
//! validates the advertised buffers and the TPM response, and never treats a
//! malformed or timed-out response as credential material. It currently
//! exposes `GetRandom` as a transport probe; stable credential roots still
//! require authenticated PCR policy and sealed NV provisioning.

use crate::{hal::acpi::Tpm2Info, paging};
use p256::ecdsa::{Signature as EcdsaSignature, VerifyingKey};
use rsa::{pkcs1v15, pss, signature::Verifier, BigUint, RsaPublicKey};
use sha2::{Digest, Sha256};

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
const TPM_ST_SESSIONS: u16 = 0x0201;
const TPM_CC_GET_RANDOM: u32 = 0x0000_017b;
const TPM_CC_READ_PUBLIC: u32 = 0x0000_0173;
#[allow(dead_code)]
const TPM_CC_QUOTE: u32 = 0x0000_0158;
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
const TPM_ALG_RSA: u16 = 0x0001;
const TPM_ALG_ECC: u16 = 0x0023;
const TPM_ALG_NULL: u16 = 0x0010;
const TPM_ECC_NIST_P256: u16 = 0x0003;
#[allow(dead_code)]
const TPM_ST_ATTEST_QUOTE: u16 = 0x8018;
#[allow(dead_code)]
const TPM_GENERATED_VALUE: u32 = 0xff54_4347;
#[allow(dead_code)]
const TPM_SESS_POLICY: u8 = 0x01;
#[allow(dead_code)]
const TPM_RH_NULL: u32 = 0x4000_0007;
#[allow(dead_code)]
const TPM_RH_OWNER: u32 = 0x4000_0001;
const TPM_RS_PW: u32 = 0x4000_0009;
#[allow(dead_code)]
const TPMA_SESSION_CONTINUE_SESSION: u8 = 1;
const TPM_RC_SUCCESS: u32 = 0;
const TPMA_NV_OWNERWRITE: u32 = 1 << 1;
const TPMA_NV_AUTHWRITE: u32 = 1 << 2;
const TPMA_NV_POLICYWRITE: u32 = 1 << 3;
const TPMA_NV_POLICY_DELETE: u32 = 1 << 22;
const TPMA_OBJECT_FIXED_TPM: u32 = 1 << 1;
const TPMA_OBJECT_FIXED_PARENT: u32 = 1 << 4;
const TPMA_OBJECT_SENSITIVE_DATA_ORIGIN: u32 = 1 << 5;
const TPMA_OBJECT_RESTRICTED: u32 = 1 << 16;
const TPMA_OBJECT_DECRYPT: u32 = 1 << 17;
const TPMA_OBJECT_SIGN_ENCRYPT: u32 = 1 << 18;
const MAX_BUFFER_BYTES: usize = 0xf80;
const PCR_POLICY_MAGIC: [u8; 4] = *b"TPP1";
const PCR_POLICY_VERSION: u16 = 1;
const PCR_POLICY_SIGNED_BYTES: usize = 44;
pub const PCR_POLICY_BYTES: usize = PCR_POLICY_SIGNED_BYTES + 64;
const ATTESTATION_NAME_POLICY_MAGIC: [u8; 4] = *b"TAN1";
const ATTESTATION_NAME_POLICY_VERSION: u16 = 1;
const ATTESTATION_NAME_POLICY_SIGNED_BYTES: usize = 42;
pub const ATTESTATION_NAME_POLICY_BYTES: usize = ATTESTATION_NAME_POLICY_SIGNED_BYTES + 64;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum QuoteSigningScheme {
    RsaSsa,
    RsaPss,
    Ecdsa,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct QuoteExecution {
    pub quote_length: u16,
    pub quote: [u8; 1024],
    pub signature_length: u16,
    pub signature: [u8; 768],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteSessionAuth<'a> {
    pub session_handle: u32,
    pub nonce_caller: &'a [u8],
    pub attributes: u8,
    pub hmac: &'a [u8],
}

/// A TPM RSA quote-signing public key in the bounded wire representation used
/// by the verification boundary. TPM's exponent value zero means 65537.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteRsaPublicKey {
    pub modulus_length: u16,
    pub modulus: [u8; 512],
    pub exponent: u32,
}

/// A TPM P-256 quote-signing public key in uncompressed affine coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteEccP256PublicKey {
    pub x: [u8; 32],
    pub y: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum AttestationPublicKey {
    Rsa(QuoteRsaPublicKey),
    EccP256(QuoteEccP256PublicKey),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttestationPublic {
    pub key: AttestationPublicKey,
    pub name: [u8; 34],
    pub qualified_name_length: u8,
    pub qualified_name: [u8; 64],
}

/// Require the returned TPM object Name to equal the provisioned identity.
#[allow(dead_code)]
pub fn bind_attestation_public_name(
    public: &AttestationPublic,
    expected_name: &[u8; 34],
) -> Result<AttestationPublicKey, Error> {
    if public.name != *expected_name {
        return Err(Error::CredentialMismatch);
    }
    Ok(public.key)
}

/// Verify a signed, bounded expected TPM attestation-object Name policy.
/// Finding a record in persistent storage does not grant authority: the
/// caller must supply the provisioned Ed25519 trust root and compare the
/// returned Name through `bind_attestation_public_name`.
#[allow(dead_code)]
pub fn verify_signed_attestation_name_policy(
    envelope: &[u8],
    trusted_key: &[u8; 32],
) -> Result<[u8; 34], Error> {
    if envelope.len() != ATTESTATION_NAME_POLICY_BYTES
        || envelope[..4] != ATTESTATION_NAME_POLICY_MAGIC
        || u16::from_be_bytes(envelope[4..6].try_into().unwrap())
            != ATTESTATION_NAME_POLICY_VERSION
        || u16::from_be_bytes(envelope[6..8].try_into().unwrap())
            != ATTESTATION_NAME_POLICY_SIGNED_BYTES as u16
    {
        return Err(Error::InvalidPolicy);
    }
    let mut name = [0_u8; 34];
    name.copy_from_slice(&envelope[8..42]);
    if name[..2] != TPM_ALG_SHA256.to_be_bytes() {
        return Err(Error::InvalidPolicy);
    }
    let mut signature = [0_u8; 64];
    signature.copy_from_slice(&envelope[ATTESTATION_NAME_POLICY_SIGNED_BYTES..]);
    crate::signed_app::verify(
        &envelope[..ATTESTATION_NAME_POLICY_SIGNED_BYTES],
        trusted_key,
        &signature,
    )
    .map_err(|_| Error::PolicySignature)?;
    Ok(name)
}

#[cfg(any(feature = "stage24-3-test", feature = "system-apps"))]
const ATTESTATION_NAME_POLICY_PATH: &str = "/wfs/tpm-attestation-name-policy";

/// Persist a signed attestation Name policy through the journal-backed WovenFS
/// writer. Signature verification happens before the durable mutation, so an
/// invalid policy can never replace a valid record.
#[cfg(any(feature = "stage24-3-test", feature = "system-apps"))]
#[allow(dead_code)]
pub fn persist_attestation_name_policy(
    authority: &crate::wfs::AuthorizedWfsWriter,
    envelope: &[u8],
    trusted_key: &[u8; 32],
    now: u64,
) -> Result<(), Error> {
    verify_signed_attestation_name_policy(envelope, trusted_key)?;
    crate::wfs::write_vfs_path_durable(authority, ATTESTATION_NAME_POLICY_PATH, envelope, now)
        .map_err(|_| Error::Storage)
}

/// Read and revalidate the durable attestation Name policy. Storage presence
/// or a valid file length never bypasses the signature check.
#[cfg(any(feature = "stage24-3-test", feature = "system-apps"))]
#[allow(dead_code)]
pub fn read_attestation_name_policy(
    trusted_key: &[u8; 32],
    envelope: &mut [u8; ATTESTATION_NAME_POLICY_BYTES],
) -> Result<[u8; 34], Error> {
    let length = crate::wfs::read_vfs_path_durable(ATTESTATION_NAME_POLICY_PATH, envelope)
        .map_err(|_| Error::Storage)?;
    if length != ATTESTATION_NAME_POLICY_BYTES {
        return Err(Error::MalformedResponse);
    }
    verify_signed_attestation_name_policy(&envelope[..length], trusted_key)
}

/// Load the provisioned attestation-object Name policy during a release boot.
///
/// The application release trust root is the only policy-signing key accepted
/// by this boot integration. A missing record, malformed record, or failed
/// signature is returned to the caller as an unavailable hardware-credential
/// capability; no fallback identity is synthesized here.
#[cfg(feature = "system-apps")]
static BOOT_ATTESTATION_NAME_POLICY: spin::Mutex<Option<[u8; ATTESTATION_NAME_POLICY_BYTES]>> =
    spin::Mutex::new(None);

#[cfg(feature = "system-apps")]
pub fn load_attestation_name_policy_at_boot() -> Result<[u8; 34], Error> {
    let mut envelope = [0_u8; ATTESTATION_NAME_POLICY_BYTES];
    let result = read_attestation_name_policy(
        &crate::signed_app::TRUSTED_PUBLIC_KEY,
        &mut envelope,
    );
    let mut loaded = BOOT_ATTESTATION_NAME_POLICY.lock();
    match result {
        Ok(name) => {
            *loaded = Some(envelope);
            Ok(name)
        }
        Err(error) => {
            *loaded = None;
            Err(error)
        }
    }
}

/// Copy the policy accepted by the boot loader for a later quote-authorizer.
/// The caller receives no policy until the boot-time signature check passed.
#[cfg(feature = "system-apps")]
pub fn boot_attestation_name_policy(
    envelope: &mut [u8; ATTESTATION_NAME_POLICY_BYTES],
) -> bool {
    let loaded = BOOT_ATTESTATION_NAME_POLICY.lock();
    let Some(policy) = *loaded else {
        return false;
    };
    envelope.copy_from_slice(&policy);
    true
}

/// Verify an ECDSA P-256/SHA-256 quote signature against the exact
/// `TPMS_ATTEST` bytes. TPM ECDSA components must be canonical 32-byte
/// integers for this curve; shorter or oversized TPM2B values are rejected.
#[allow(dead_code)]
pub fn verify_quote_ecc_p256_sha256(
    quote: &[u8],
    signature: &QuoteSignature,
    public_key: &QuoteEccP256PublicKey,
) -> Result<(), Error> {
    if signature.algorithm != TPM_ALG_ECDSA
        || signature.hash_algorithm != TPM_ALG_SHA256
        || signature.first_length != 32
        || signature.second_length != 32
    {
        return Err(Error::InvalidPolicy);
    }
    let mut encoded_key = [0_u8; 65];
    encoded_key[0] = 4;
    encoded_key[1..33].copy_from_slice(&public_key.x);
    encoded_key[33..].copy_from_slice(&public_key.y);
    let verifying_key = VerifyingKey::from_sec1_bytes(&encoded_key)
        .map_err(|_| Error::InvalidPolicy)?;
    let mut r = [0_u8; 32];
    r.copy_from_slice(&signature.first[..32]);
    let mut s = [0_u8; 32];
    s.copy_from_slice(&signature.second[..32]);
    let signature = EcdsaSignature::from_scalars(r, s).map_err(|_| Error::MalformedResponse)?;
    verifying_key
        .verify(quote, &signature)
        .map_err(|_| Error::PolicySignature)
}

/// Verify an RSA SHA-256 quote signature against the exact TPMS_ATTEST bytes.
/// The caller must obtain and validate this public key from the TPM before
/// using the result for any credential or boot-authority decision.
#[allow(dead_code)]
pub fn verify_quote_rsa_sha256(
    quote: &[u8],
    signature: &QuoteSignature,
    public_key: &QuoteRsaPublicKey,
) -> Result<(), Error> {
    if signature.hash_algorithm != TPM_ALG_SHA256
        || !matches!(signature.algorithm, TPM_ALG_RSASSA | TPM_ALG_RSAPSS)
        || public_key.modulus_length < 256
        || public_key.modulus_length > 512
        || !public_key.modulus_length.is_multiple_of(8)
    {
        return Err(Error::InvalidPolicy);
    }
    let modulus_length = usize::from(public_key.modulus_length);
    let modulus = BigUint::from_bytes_be(&public_key.modulus[..modulus_length]);
    let exponent = BigUint::from(if public_key.exponent == 0 {
        65_537
    } else {
        public_key.exponent
    });
    let key = RsaPublicKey::new(modulus, exponent).map_err(|_| Error::InvalidPolicy)?;
    let signature_length = usize::from(signature.first_length);
    if signature_length == 0 || signature_length != modulus_length {
        return Err(Error::MalformedResponse);
    }
    let signature_bytes = &signature.first[..signature_length];
    if signature.algorithm == TPM_ALG_RSASSA {
        let verifying_key = pkcs1v15::VerifyingKey::<Sha256>::new(key);
        let parsed = pkcs1v15::Signature::try_from(signature_bytes)
            .map_err(|_| Error::MalformedResponse)?;
        verifying_key
            .verify(quote, &parsed)
            .map_err(|_| Error::PolicySignature)
    } else {
        let verifying_key = pss::VerifyingKey::<Sha256>::new(key);
        let parsed = pss::Signature::try_from(signature_bytes)
            .map_err(|_| Error::MalformedResponse)?;
        verifying_key
            .verify(quote, &parsed)
            .map_err(|_| Error::PolicySignature)
    }
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

/// Authorize one TPM quote only when every independent binding agrees:
/// signed expected Name, validated `ReadPublic` Name, nonce/PCR evidence, and
/// cryptographic quote signature. No caller-supplied key can bypass the Name
/// policy because the key is selected from the validated public-area result.
pub struct QuoteAuthorizationRequest<'a> {
    pub name_policy: &'a [u8],
    pub trusted_policy_key: &'a [u8; 32],
    pub public: &'a AttestationPublic,
    pub quote: &'a [u8],
    pub signature: &'a [u8],
    pub expected_nonce: &'a [u8],
    pub expected_pcr: u8,
    pub expected_digest: &'a [u8; 32],
}

#[allow(dead_code)]
pub fn authorize_quote(request: QuoteAuthorizationRequest<'_>) -> Result<QuoteAttestation, Error> {
    let expected_name = verify_signed_attestation_name_policy(
        request.name_policy,
        request.trusted_policy_key,
    )?;
    let key = bind_attestation_public_name(request.public, &expected_name)?;
    let attestation = decode_quote_attestation(
        request.quote,
        request.expected_nonce,
        request.expected_pcr,
        request.expected_digest,
    )?;
    let (signature, consumed) = decode_quote_signature(request.signature)?;
    if consumed != request.signature.len() {
        return Err(Error::MalformedResponse);
    }
    match key {
        AttestationPublicKey::Rsa(public_key) => {
            if signature.algorithm != TPM_ALG_RSASSA && signature.algorithm != TPM_ALG_RSAPSS {
                return Err(Error::InvalidPolicy);
            }
            verify_quote_rsa_sha256(request.quote, &signature, &public_key)?;
        }
        AttestationPublicKey::EccP256(public_key) => {
            if signature.algorithm != TPM_ALG_ECDSA {
                return Err(Error::InvalidPolicy);
            }
            verify_quote_ecc_p256_sha256(request.quote, &signature, &public_key)?;
        }
    }
    Ok(attestation)
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
    #[allow(dead_code)]
    Storage,
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

#[allow(dead_code)]
fn encode_quote_session(
    signing_handle: u32,
    qualifying_data: &[u8],
    pcr: u8,
    scheme: QuoteSigningScheme,
    session: QuoteSessionAuth<'_>,
    output: &mut [u8; 192],
) -> Result<usize, Error> {
    if signing_handle == 0
        || qualifying_data.len() > 32
        || pcr >= 24
        || session.session_handle == 0
        || session.nonce_caller.len() > 32
        || session.hmac.len() > 64
    {
        return Err(Error::InvalidDescriptor);
    }
    let scheme_algorithm = match scheme {
        QuoteSigningScheme::RsaSsa => TPM_ALG_RSASSA,
        QuoteSigningScheme::RsaPss => TPM_ALG_RSAPSS,
        QuoteSigningScheme::Ecdsa => TPM_ALG_ECDSA,
    };
    output.fill(0);
    output[..2].copy_from_slice(&TPM_ST_SESSIONS.to_be_bytes());
    output[6..10].copy_from_slice(&TPM_CC_QUOTE.to_be_bytes());
    output[10..14].copy_from_slice(&signing_handle.to_be_bytes());
    // The password session is deliberately empty. The TPM object policy and
    // its sign/encrypt attributes remain the authority; this session only
    // frames the command and cannot create a credential root.
    let auth_size = 9usize
        .saturating_add(session.nonce_caller.len())
        .saturating_add(session.hmac.len());
    output[14..18].copy_from_slice(&(auth_size as u32).to_be_bytes());
    output[18..22].copy_from_slice(&session.session_handle.to_be_bytes());
    output[22..24].copy_from_slice(&(session.nonce_caller.len() as u16).to_be_bytes());
    let nonce_end = 24 + session.nonce_caller.len();
    output[24..nonce_end].copy_from_slice(session.nonce_caller);
    output[nonce_end] = session.attributes;
    output[nonce_end + 1..nonce_end + 3]
        .copy_from_slice(&(session.hmac.len() as u16).to_be_bytes());
    output[nonce_end + 3..nonce_end + 3 + session.hmac.len()].copy_from_slice(session.hmac);
    let mut offset = nonce_end + 3 + session.hmac.len();
    output[offset..offset + 2].copy_from_slice(&(qualifying_data.len() as u16).to_be_bytes());
    offset += 2;
    output[offset..offset + qualifying_data.len()].copy_from_slice(qualifying_data);
    offset += qualifying_data.len();
    output[offset..offset + 2].copy_from_slice(&scheme_algorithm.to_be_bytes());
    output[offset + 2..offset + 4].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
    offset += 4;
    output[offset..offset + 4].copy_from_slice(&1_u32.to_be_bytes());
    output[offset + 4..offset + 6].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
    output[offset + 6] = 3;
    output[offset + 7 + usize::from(pcr / 8)] = 1 << (pcr % 8);
    offset += 10;
    output[2..6].copy_from_slice(&(offset as u32).to_be_bytes());
    Ok(offset)
}

#[allow(dead_code)]
fn encode_quote(
    signing_handle: u32,
    qualifying_data: &[u8],
    pcr: u8,
    scheme: QuoteSigningScheme,
    output: &mut [u8; 192],
) -> Result<usize, Error> {
    encode_quote_session(
        signing_handle,
        qualifying_data,
        pcr,
        scheme,
        QuoteSessionAuth {
            session_handle: TPM_RS_PW,
            nonce_caller: &[],
            attributes: 0,
            hmac: &[],
        },
        output,
    )
}

#[allow(dead_code)]
fn decode_quote_response(
    response: &[u8],
    output: &mut QuoteExecution,
) -> Result<(), Error> {
    if response.len() < 19
        || u16::from_be_bytes(response[..2].try_into().unwrap()) != TPM_ST_SESSIONS
    {
        return Err(Error::MalformedResponse);
    }
    let response_size = u32::from_be_bytes(response[2..6].try_into().unwrap()) as usize;
    let response_code = u32::from_be_bytes(response[6..10].try_into().unwrap());
    if response_size != response.len() {
        return Err(Error::MalformedResponse);
    }
    if response_code != TPM_RC_SUCCESS {
        return Err(Error::Device(response_code));
    }
    let parameter_size = u32::from_be_bytes(response[10..14].try_into().unwrap()) as usize;
    if parameter_size < 2 || 14usize.saturating_add(parameter_size) > response.len() {
        return Err(Error::MalformedResponse);
    }
    let parameter_end = 14 + parameter_size;
    let mut offset = 14;
    let quote_length = u16::from_be_bytes(response[offset..offset + 2].try_into().unwrap()) as usize;
    offset += 2;
    if quote_length == 0 || quote_length > output.quote.len() || offset + quote_length > parameter_end {
        return Err(Error::MalformedResponse);
    }
    output.quote[..quote_length].copy_from_slice(&response[offset..offset + quote_length]);
    offset += quote_length;
    let signature_start = offset;
    if signature_start + 2 > parameter_end {
        return Err(Error::MalformedResponse);
    }
    let signature_algorithm =
        u16::from_be_bytes(response[signature_start..signature_start + 2].try_into().unwrap());
    let signature_length = match signature_algorithm {
        TPM_ALG_RSASSA | TPM_ALG_RSAPSS => {
            if signature_start + 6 > parameter_end {
                return Err(Error::MalformedResponse);
            }
            6 + u16::from_be_bytes(
                response[signature_start + 4..signature_start + 6]
                    .try_into()
                    .unwrap(),
            ) as usize
        }
        TPM_ALG_ECDSA => {
            if signature_start + 6 > parameter_end {
                return Err(Error::MalformedResponse);
            }
            let r_length = u16::from_be_bytes(
                response[signature_start + 4..signature_start + 6]
                    .try_into()
                    .unwrap(),
            ) as usize;
            if signature_start + 6 + r_length + 2 > parameter_end {
                return Err(Error::MalformedResponse);
            }
            8 + r_length
                + u16::from_be_bytes(
                    response[signature_start + 6 + r_length..signature_start + 8 + r_length]
                        .try_into()
                        .unwrap(),
                ) as usize
        }
        _ => return Err(Error::UnsupportedInterface),
    };
    if signature_length == 0
        || signature_length > output.signature.len()
        || signature_start + signature_length != parameter_end
    {
        return Err(Error::MalformedResponse);
    }
    output.signature[..signature_length]
        .copy_from_slice(&response[signature_start..parameter_end]);
    let auth = &response[parameter_end..];
    if auth.len() != 5 || u16::from_be_bytes(auth[..2].try_into().unwrap()) != 0 || auth[2] != 0 {
        return Err(Error::MalformedResponse);
    }
    if u16::from_be_bytes(auth[3..5].try_into().unwrap()) != 0 {
        return Err(Error::MalformedResponse);
    }
    output.quote_length = quote_length as u16;
    output.signature_length = signature_length as u16;
    Ok(())
}

fn execute_quote(
    info: Tpm2Info,
    signing_handle: u32,
    qualifying_data: &[u8],
    pcr: u8,
    scheme: QuoteSigningScheme,
    session: QuoteSessionAuth<'_>,
) -> Result<QuoteExecution, Error> {
    let mut command = [0_u8; 192];
    let command_size = encode_quote_session(
        signing_handle,
        qualifying_data,
        pcr,
        scheme,
        session,
        &mut command,
    )?;
    let mut response = [0_u8; MAX_BUFFER_BYTES];
    let response_size = execute(info, &command[..command_size], &mut response)?;
    let mut output = QuoteExecution {
        quote_length: 0,
        quote: [0; 1024],
        signature_length: 0,
        signature: [0; 768],
    };
    decode_quote_response(&response[..response_size], &mut output)?;
    Ok(output)
}

/// Execute one bounded TPM2_Quote using an empty password session. The result
/// is only wire evidence; callers must pass it through `authorize_quote`
/// before using it for any credential decision.
#[allow(dead_code)]
pub fn quote(
    info: Tpm2Info,
    signing_handle: u32,
    qualifying_data: &[u8],
    pcr: u8,
    scheme: QuoteSigningScheme,
) -> Result<QuoteExecution, Error> {
    execute_quote(
        info,
        signing_handle,
        qualifying_data,
        pcr,
        scheme,
        QuoteSessionAuth {
            session_handle: TPM_RS_PW,
            nonce_caller: &[],
            attributes: 0,
            hmac: &[],
        },
    )
}

/// Execute `TPM2_Quote` with a caller-established policy session. The caller
/// supplies the TPM session nonce, attributes, and authorization HMAC; this
/// function never fabricates or downgrades those authorization inputs.
#[allow(dead_code)]
pub fn quote_with_policy_session(
    info: Tpm2Info,
    signing_handle: u32,
    qualifying_data: &[u8],
    pcr: u8,
    scheme: QuoteSigningScheme,
    session: QuoteSessionAuth<'_>,
) -> Result<QuoteExecution, Error> {
    if session.session_handle == TPM_RS_PW {
        return Err(Error::InvalidPolicy);
    }
    execute_quote(
        info,
        signing_handle,
        qualifying_data,
        pcr,
        scheme,
        session,
    )
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

/// Start a TPM policy session through the serialized CRB transport. The
/// caller nonce is copied into the bounded command and the returned TPM nonce
/// must be retained for any later session-HMAC calculation.
#[allow(dead_code)]
pub fn start_policy_session(
    info: Tpm2Info,
    nonce_caller: &[u8],
) -> Result<PolicySession, Error> {
    let mut command = [0_u8; 95];
    let command_size = encode_policy_session_start(nonce_caller, &mut command)?;
    let mut response = [0_u8; MAX_BUFFER_BYTES];
    let response_size = execute(info, &command[..command_size], &mut response)?;
    decode_policy_session_start(&response[..response_size])
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

/// Extend a live policy session with a SHA-256 PCR assertion. This command
/// changes TPM session state but returns no credential authority; the caller
/// must retain the session handle and obtain/derive the updated session
/// material before using it to authorize `TPM2_Quote`.
#[allow(dead_code)]
pub fn apply_policy_pcr(
    info: Tpm2Info,
    session_handle: u32,
    pcr: u8,
    expected_digest: &[u8; 32],
) -> Result<(), Error> {
    let mut command = [0_u8; 58];
    encode_policy_pcr(session_handle, pcr, expected_digest, &mut command)?;
    let mut response = [0_u8; 16];
    let response_size = execute(info, &command, &mut response)?;
    decode_policy_command_response(&response[..response_size])
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

fn skip_tpm2b<'a>(bytes: &'a [u8], offset: &mut usize, maximum: usize) -> Result<&'a [u8], Error> {
    let length = u16::from_be_bytes(quote_take(bytes, offset, 2)?.try_into().unwrap()) as usize;
    if length > maximum {
        return Err(Error::MalformedResponse);
    }
    quote_take(bytes, offset, length)
}

fn decode_public_symmetric(bytes: &[u8], offset: &mut usize) -> Result<(), Error> {
    let algorithm = u16::from_be_bytes(quote_take(bytes, offset, 2)?.try_into().unwrap());
    if algorithm != TPM_ALG_NULL {
        quote_take(bytes, offset, 4)?;
    }
    Ok(())
}

fn validate_attestation_attributes(attributes: u32) -> Result<(), Error> {
    let required = TPMA_OBJECT_FIXED_TPM
        | TPMA_OBJECT_FIXED_PARENT
        | TPMA_OBJECT_SENSITIVE_DATA_ORIGIN
        | TPMA_OBJECT_RESTRICTED
        | TPMA_OBJECT_SIGN_ENCRYPT;
    if attributes & required != required || attributes & TPMA_OBJECT_DECRYPT != 0 {
        return Err(Error::InvalidPolicy);
    }
    Ok(())
}

/// Decode and validate the public area returned by `TPM2_ReadPublic` for an
/// attestation/signing object. This is a provenance boundary only: the
/// caller must still bind the returned object name to the expected TPM object
/// and use the object handle under a live TPM authorization policy.
#[allow(dead_code)]
pub fn decode_attestation_public(response: &[u8]) -> Result<AttestationPublic, Error> {
    if response.len() < 18 {
        return Err(Error::MalformedResponse);
    }
    let response_size = u32::from_be_bytes(response[2..6].try_into().unwrap()) as usize;
    let response_code = u32::from_be_bytes(response[6..10].try_into().unwrap());
    if response_size != response.len() {
        return Err(Error::MalformedResponse);
    }
    if response_code != TPM_RC_SUCCESS {
        return Err(Error::Device(response_code));
    }
    let public_size = u16::from_be_bytes(response[10..12].try_into().unwrap()) as usize;
    let public_end = 12usize.saturating_add(public_size);
    if public_size == 0 || public_end > response_size {
        return Err(Error::MalformedResponse);
    }
    let public = &response[12..public_end];
    let mut offset = 0;
    let public_type = u16::from_be_bytes(quote_take(public, &mut offset, 2)?.try_into().unwrap());
    let name_algorithm = u16::from_be_bytes(quote_take(public, &mut offset, 2)?.try_into().unwrap());
    let attributes = u32::from_be_bytes(quote_take(public, &mut offset, 4)?.try_into().unwrap());
    if name_algorithm != TPM_ALG_SHA256 {
        return Err(Error::InvalidPolicy);
    }
    validate_attestation_attributes(attributes)?;
    skip_tpm2b(public, &mut offset, 64)?;
    let key = match public_type {
        TPM_ALG_RSA => {
            decode_public_symmetric(public, &mut offset)?;
            quote_take(public, &mut offset, 4)?;
            let key_bits = u16::from_be_bytes(quote_take(public, &mut offset, 2)?.try_into().unwrap());
            let exponent = u32::from_be_bytes(quote_take(public, &mut offset, 4)?.try_into().unwrap());
            if !(2048..=4096).contains(&key_bits) || key_bits % 8 != 0 {
                return Err(Error::InvalidPolicy);
            }
            let modulus_length = usize::from(key_bits / 8);
            let modulus = skip_tpm2b(public, &mut offset, 512)?;
            if modulus.len() != modulus_length {
                return Err(Error::InvalidPolicy);
            }
            let exponent_value = if exponent == 0 { 65_537 } else { exponent };
            RsaPublicKey::new(
                BigUint::from_bytes_be(modulus),
                BigUint::from(exponent_value),
            )
            .map_err(|_| Error::InvalidPolicy)?;
            let mut public_key = QuoteRsaPublicKey {
                modulus_length: key_bits / 8,
                modulus: [0; 512],
                exponent,
            };
            public_key.modulus[..modulus_length].copy_from_slice(modulus);
            AttestationPublicKey::Rsa(public_key)
        }
        TPM_ALG_ECC => {
            decode_public_symmetric(public, &mut offset)?;
            quote_take(public, &mut offset, 4)?;
            let curve = u16::from_be_bytes(quote_take(public, &mut offset, 2)?.try_into().unwrap());
            quote_take(public, &mut offset, 4)?;
            if curve != TPM_ECC_NIST_P256 {
                return Err(Error::UnsupportedInterface);
            }
            let x = skip_tpm2b(public, &mut offset, 32)?;
            let y = skip_tpm2b(public, &mut offset, 32)?;
            if x.len() != 32 || y.len() != 32 {
                return Err(Error::InvalidPolicy);
            }
            let mut public_key = QuoteEccP256PublicKey { x: [0; 32], y: [0; 32] };
            public_key.x.copy_from_slice(x);
            public_key.y.copy_from_slice(y);
            if VerifyingKey::from_sec1_bytes(&{
                let mut encoded = [0_u8; 65];
                encoded[0] = 4;
                encoded[1..33].copy_from_slice(&public_key.x);
                encoded[33..].copy_from_slice(&public_key.y);
                encoded
            })
            .is_err()
            {
                return Err(Error::InvalidPolicy);
            }
            AttestationPublicKey::EccP256(public_key)
        }
        _ => return Err(Error::UnsupportedInterface),
    };
    if offset != public.len() {
        return Err(Error::MalformedResponse);
    }
    let mut tail = public_end;
    let name = skip_tpm2b(response, &mut tail, 64)?;
    if name.len() != 34 || u16::from_be_bytes(name[..2].try_into().unwrap()) != TPM_ALG_SHA256 {
        return Err(Error::InvalidPolicy);
    }
    let digest = Sha256::digest(public);
    if name[2..] != digest[..] {
        return Err(Error::CredentialMismatch);
    }
    let qualified_name = skip_tpm2b(response, &mut tail, 64)?;
    if tail != response_size {
        return Err(Error::MalformedResponse);
    }
    let mut parsed_name = [0_u8; 34];
    parsed_name.copy_from_slice(name);
    let mut parsed_qualified_name = [0_u8; 64];
    parsed_qualified_name[..qualified_name.len()].copy_from_slice(qualified_name);
    Ok(AttestationPublic {
        key,
        name: parsed_name,
        qualified_name_length: qualified_name.len() as u8,
        qualified_name: parsed_qualified_name,
    })
}

#[allow(dead_code)]
fn encode_read_public(handle: u32, output: &mut [u8; 14]) -> Result<usize, Error> {
    output[..2].copy_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
    output[2..6].copy_from_slice(&14_u32.to_be_bytes());
    output[6..10].copy_from_slice(&TPM_CC_READ_PUBLIC.to_be_bytes());
    output[10..14].copy_from_slice(&handle.to_be_bytes());
    Ok(output.len())
}

/// Retrieve one bounded TPM public area through the serialized CRB transport.
#[allow(dead_code)]
pub fn read_attestation_public(
    info: Tpm2Info,
    handle: u32,
) -> Result<AttestationPublic, Error> {
    let mut command = [0_u8; 14];
    let command_size = encode_read_public(handle, &mut command)?;
    let mut response = [0_u8; MAX_BUFFER_BYTES];
    let response_size = execute(info, &command[..command_size], &mut response)?;
    decode_attestation_public(&response[..response_size])
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
    let quote_nonce = [0x5a; 16];
    let mut quote_command = [0_u8; 192];
    let Ok(quote_command_size) = encode_quote(
        0x8101_0001,
        &quote_nonce,
        7,
        QuoteSigningScheme::RsaSsa,
        &mut quote_command,
    ) else {
        return false;
    };
    if quote_command_size != 59
        || u16::from_be_bytes(quote_command[..2].try_into().unwrap()) != TPM_ST_SESSIONS
        || u32::from_be_bytes(quote_command[6..10].try_into().unwrap()) != TPM_CC_QUOTE
        || u32::from_be_bytes(quote_command[10..14].try_into().unwrap()) != 0x8101_0001
        || u32::from_be_bytes(quote_command[14..18].try_into().unwrap()) != 9
        || u32::from_be_bytes(quote_command[18..22].try_into().unwrap()) != TPM_RS_PW
        || u16::from_be_bytes(quote_command[27..29].try_into().unwrap()) != quote_nonce.len() as u16
        || quote_command[29..45] != quote_nonce
        || u16::from_be_bytes(quote_command[45..47].try_into().unwrap()) != TPM_ALG_RSASSA
        || u16::from_be_bytes(quote_command[47..49].try_into().unwrap()) != TPM_ALG_SHA256
        || u32::from_be_bytes(quote_command[49..53].try_into().unwrap()) != 1
        || u16::from_be_bytes(quote_command[53..55].try_into().unwrap()) != TPM_ALG_SHA256
        || quote_command[55] != 3
        || quote_command[56] != 0x80
    {
        return false;
    }
    let policy_nonce = [0x11; 16];
    let policy_hmac = [0x22; 32];
    let mut policy_quote_command = [0_u8; 192];
    let Ok(policy_command_size) = encode_quote_session(
        0x8101_0001,
        &quote_nonce,
        7,
        QuoteSigningScheme::Ecdsa,
        QuoteSessionAuth {
            session_handle: 0x0300_0000,
            nonce_caller: &policy_nonce,
            attributes: TPMA_SESSION_CONTINUE_SESSION,
            hmac: &policy_hmac,
        },
        &mut policy_quote_command,
    ) else {
        return false;
    };
    if policy_command_size != 107
        || u32::from_be_bytes(policy_quote_command[14..18].try_into().unwrap()) != 57
        || u32::from_be_bytes(policy_quote_command[18..22].try_into().unwrap()) != 0x0300_0000
        || u16::from_be_bytes(policy_quote_command[22..24].try_into().unwrap()) != 16
        || policy_quote_command[24..40] != policy_nonce
        || policy_quote_command[40] != TPMA_SESSION_CONTINUE_SESSION
        || u16::from_be_bytes(policy_quote_command[41..43].try_into().unwrap()) != 32
        || policy_quote_command[43..75] != policy_hmac
        || u16::from_be_bytes(policy_quote_command[75..77].try_into().unwrap()) != 16
        || policy_quote_command[77..93] != quote_nonce
        || u16::from_be_bytes(policy_quote_command[93..95].try_into().unwrap()) != TPM_ALG_ECDSA
    {
        return false;
    }
    let mut quote_response = [0_u8; 378];
    let quote_response_size = quote_response.len() as u32;
    quote_response[..2].copy_from_slice(&TPM_ST_SESSIONS.to_be_bytes());
    quote_response[2..6].copy_from_slice(&quote_response_size.to_be_bytes());
    quote_response[10..14].copy_from_slice(&359_u32.to_be_bytes());
    quote_response[14..16].copy_from_slice(&95_u16.to_be_bytes());
    quote_response[16..111].fill(0x41);
    quote_response[111..113].copy_from_slice(&TPM_ALG_RSASSA.to_be_bytes());
    quote_response[113..115].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
    quote_response[115..117].copy_from_slice(&256_u16.to_be_bytes());
    quote_response[117..373].fill(0x7e);
    let mut quote_output = QuoteExecution {
        quote_length: 0,
        quote: [0; 1024],
        signature_length: 0,
        signature: [0; 768],
    };
    if decode_quote_response(&quote_response, &mut quote_output).is_err()
        || quote_output.quote_length != 95
        || quote_output.signature_length != 262
        || quote_output.quote[..95] != [0x41; 95]
        || quote_output.signature[..262] != quote_response[111..373]
    {
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
            let rsa_message = b"WovenHat TPM quote RSA verification fixture";
            let mut rsa_key = QuoteRsaPublicKey {
                modulus_length: 256,
                modulus: [0; 512],
                exponent: 65_537,
            };
            rsa_key.modulus[..256].copy_from_slice(&[
                    0xbd, 0x77, 0x2e, 0x1b, 0x39, 0x72, 0x8a, 0x8b, 0x5f, 0x3e, 0x1c, 0xe2, 0x35, 0x2c, 0xbb, 0xaa,
                    0xad, 0x67, 0x7f, 0x2d, 0x3d, 0xcd, 0xe6, 0x37, 0xe6, 0x18, 0x1c, 0x90, 0xee, 0xc7, 0x52, 0xc1,
                    0x04, 0x0c, 0x4f, 0xe9, 0xb1, 0x80, 0xd8, 0x91, 0xeb, 0x89, 0xc4, 0x3e, 0x72, 0x39, 0xa5, 0x4e,
                    0xaf, 0x21, 0x2c, 0x2f, 0xe4, 0x58, 0xe5, 0x44, 0xd2, 0x8a, 0x8f, 0x6a, 0xa3, 0x7f, 0xb7, 0xc6,
                    0xeb, 0x3c, 0xbc, 0x23, 0xb0, 0x5f, 0x63, 0xf1, 0xce, 0x46, 0x9f, 0x8d, 0xfa, 0x9f, 0x51, 0x69,
                    0xa9, 0x4b, 0xe9, 0x32, 0x2f, 0x9b, 0x09, 0xb0, 0xca, 0xf8, 0x49, 0x0f, 0x3f, 0x64, 0x13, 0x19,
                    0x83, 0xce, 0x17, 0x9c, 0xe7, 0xae, 0x69, 0x7b, 0x6e, 0x42, 0x18, 0xd0, 0xaa, 0x91, 0xe4, 0x7f,
                    0x3b, 0x2d, 0x84, 0xdd, 0x65, 0xd1, 0x1b, 0xd1, 0xc9, 0x9c, 0xc1, 0x84, 0x69, 0x0b, 0x8a, 0xfb,
                    0xa4, 0x6f, 0x91, 0xe8, 0xc4, 0xe5, 0x8d, 0xb6, 0x9e, 0xc5, 0x66, 0x45, 0xe8, 0x85, 0x30, 0x86,
                    0xff, 0xe7, 0xba, 0xd7, 0x1f, 0x18, 0xda, 0xda, 0xfe, 0x5b, 0x4f, 0xd0, 0x43, 0x1a, 0xf4, 0x42,
                    0xc5, 0x6b, 0x64, 0x8e, 0x18, 0x53, 0x67, 0xa1, 0x13, 0x02, 0xbe, 0xe3, 0x1d, 0x2d, 0xee, 0x96,
                    0xba, 0xdb, 0x15, 0x1d, 0x14, 0xec, 0x4d, 0x51, 0x11, 0x7e, 0x89, 0x38, 0x4a, 0x3e, 0xda, 0xe7,
                    0xae, 0x14, 0x6d, 0x53, 0x57, 0x7c, 0x98, 0xf3, 0x4b, 0x1b, 0xda, 0xe4, 0x4e, 0x21, 0x94, 0x3b,
                    0x7c, 0x73, 0x76, 0x47, 0xf9, 0xc4, 0xf5, 0xe5, 0x61, 0x1c, 0x87, 0xcd, 0x8f, 0xeb, 0x56, 0xcf,
                    0x9c, 0x3a, 0x5f, 0x74, 0x46, 0x0d, 0x6a, 0x75, 0x26, 0xe3, 0x1f, 0x59, 0x5e, 0x4d, 0x8e, 0x03,
                    0xc9, 0x25, 0x58, 0x3e, 0xea, 0x7a, 0x88, 0x15, 0xae, 0x78, 0xd5, 0x21, 0x19, 0xf0, 0x9d, 0xf7,
            ]);
            let mut rsa_fixture_signature = QuoteSignature {
                algorithm: TPM_ALG_RSASSA,
                hash_algorithm: TPM_ALG_SHA256,
                first_length: 256,
                first: [0; 512],
                second_length: 0,
                second: [0; 72],
            };
            rsa_fixture_signature.first[..256].copy_from_slice(&[
                0x72, 0xb7, 0x76, 0x94, 0x98, 0x46, 0x01, 0x9a, 0xd3, 0xa0, 0xd2, 0xf5, 0x16, 0x0c, 0x2e, 0xcc,
                0x43, 0xb3, 0x30, 0xe9, 0xea, 0x84, 0x1b, 0x24, 0x87, 0x97, 0xc3, 0x1e, 0x7a, 0xe1, 0x81, 0x65,
                0x25, 0x3f, 0x3f, 0x6c, 0x63, 0x88, 0x9d, 0xca, 0x3d, 0x77, 0x6e, 0xab, 0x94, 0x7f, 0xbf, 0x99,
                0x1f, 0x07, 0x14, 0xb1, 0x64, 0x9b, 0x71, 0x0d, 0xb1, 0xef, 0x32, 0x62, 0xfd, 0x71, 0xe6, 0x86,
                0xc4, 0xd8, 0xc9, 0x64, 0x52, 0xd7, 0xd0, 0x85, 0x9b, 0x96, 0x26, 0xbc, 0x05, 0x60, 0x59, 0x3c,
                0x48, 0x5b, 0x49, 0x13, 0xe3, 0x56, 0xc0, 0xcb, 0x6d, 0x7f, 0xfc, 0xd1, 0xc5, 0x63, 0x3b, 0xa7,
                0x86, 0x27, 0x88, 0x8a, 0x36, 0x73, 0x1b, 0x1a, 0x8d, 0xa8, 0xfc, 0x61, 0x3a, 0x21, 0x86, 0x0a,
                0xfb, 0xe4, 0xbf, 0x52, 0x50, 0x6a, 0x03, 0x64, 0x83, 0xd8, 0x51, 0xaa, 0xd1, 0x12, 0x17, 0x04,
                0x67, 0x5c, 0x16, 0x14, 0x72, 0x02, 0x3c, 0xb6, 0xd9, 0x6e, 0x06, 0xe7, 0xf6, 0x03, 0x94, 0x7b,
                0xf5, 0x86, 0x9f, 0xd0, 0x10, 0xeb, 0xb6, 0x5c, 0x8d, 0xc3, 0x2d, 0x08, 0x4f, 0xfa, 0xb9, 0xcb,
                0x81, 0x4a, 0x9a, 0x09, 0xff, 0x70, 0x16, 0x7d, 0x15, 0x2a, 0x96, 0x47, 0x61, 0xbf, 0xf1, 0x11,
                0x0d, 0xe7, 0xe0, 0x2a, 0x9d, 0xdc, 0xcc, 0x19, 0x5f, 0x72, 0x66, 0xd4, 0x24, 0x0a, 0x93, 0xaa,
                0x6b, 0xb0, 0x3e, 0x5f, 0xa9, 0x4e, 0x79, 0x44, 0x68, 0xf7, 0x5b, 0xff, 0x6c, 0xb6, 0x5e, 0x1c,
                0xa4, 0xc7, 0x17, 0x75, 0xbc, 0x58, 0xfc, 0x4a, 0xb1, 0xb4, 0x60, 0x61, 0x2a, 0x37, 0x3e, 0x31,
                0x90, 0x55, 0xc6, 0x5f, 0xcd, 0x2d, 0x09, 0x2a, 0x94, 0x58, 0x2a, 0x49, 0x57, 0x91, 0xfd, 0x2b,
                0xb4, 0x46, 0xd6, 0xbd, 0x07, 0x54, 0x04, 0x5c, 0xfe, 0x17, 0x68, 0x29, 0x18, 0x56, 0x7d, 0x1c,
            ]);
            if verify_quote_rsa_sha256(rsa_message, &rsa_fixture_signature, &rsa_key).is_err() {
                return false;
            }
            rsa_fixture_signature.first[0] ^= 1;
            if verify_quote_rsa_sha256(rsa_message, &rsa_fixture_signature, &rsa_key)
                != Err(Error::PolicySignature)
            {
                return false;
            }
            let ecc_message = b"WovenHat TPM quote P-256 verification fixture";
            let ecc_key = QuoteEccP256PublicKey {
                x: [
                    0x6f, 0xf0, 0x3b, 0x94, 0x92, 0x41, 0xce, 0x1d, 0xad, 0xd4, 0x35, 0x19, 0xe6, 0x96, 0x0e, 0x0a,
                    0x85, 0xb4, 0x1a, 0x69, 0xa0, 0x5c, 0x32, 0x81, 0x03, 0xaa, 0x2b, 0xce, 0x15, 0x94, 0xca, 0x16,
                ],
                y: [
                    0x3c, 0x4f, 0x75, 0x3a, 0x55, 0xbf, 0x01, 0xdc, 0x53, 0xf6, 0xc0, 0xb0, 0xc7, 0xee, 0xe7, 0x8b,
                    0x40, 0xc6, 0xff, 0x7d, 0x25, 0xa9, 0x6e, 0x22, 0x82, 0xb9, 0x89, 0xce, 0xf7, 0x1c, 0x14, 0x4a,
                ],
            };
            let mut ecc_fixture_signature = QuoteSignature {
                algorithm: TPM_ALG_ECDSA,
                hash_algorithm: TPM_ALG_SHA256,
                first_length: 32,
                first: [0; 512],
                second_length: 32,
                second: [0; 72],
            };
            ecc_fixture_signature.first[..32].copy_from_slice(&[
                0x8c, 0x0c, 0x73, 0xa8, 0x5a, 0xb1, 0x46, 0xca, 0xc5, 0xc0, 0x27, 0xf1, 0x48, 0x22, 0xa2, 0xf8,
                0x26, 0xca, 0x3a, 0xf2, 0xbd, 0xd5, 0x67, 0xe1, 0xf3, 0x20, 0x91, 0x0e, 0x83, 0x96, 0x5b, 0x18,
            ]);
            ecc_fixture_signature.second[..32].copy_from_slice(&[
                0x5a, 0x67, 0xd4, 0xc7, 0xfa, 0xd8, 0x6c, 0xcc, 0x20, 0x6e, 0xb9, 0xa6, 0xa2, 0x1d, 0xd8, 0x9f,
                0xbc, 0xfe, 0x6b, 0x3e, 0x1c, 0xb4, 0x62, 0xd8, 0x37, 0xb1, 0xdf, 0x08, 0x83, 0x94, 0x04, 0x13,
            ]);
            if verify_quote_ecc_p256_sha256(ecc_message, &ecc_fixture_signature, &ecc_key).is_err() {
                return false;
            }
            ecc_fixture_signature.second[0] ^= 1;
            if verify_quote_ecc_p256_sha256(ecc_message, &ecc_fixture_signature, &ecc_key)
                != Err(Error::PolicySignature)
            {
                return false;
            }
            let mut public_response = [0_u8; 330];
            public_response[..2].copy_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
            public_response[2..6].copy_from_slice(&330_u32.to_be_bytes());
            public_response[6..10].copy_from_slice(&TPM_RC_SUCCESS.to_be_bytes());
            public_response[10..12].copy_from_slice(&280_u16.to_be_bytes());
            public_response[12..14].copy_from_slice(&TPM_ALG_RSA.to_be_bytes());
            public_response[14..16].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
            public_response[16..20].copy_from_slice(&0x0005_0032_u32.to_be_bytes());
            public_response[20..22].copy_from_slice(&0_u16.to_be_bytes());
            public_response[22..24].copy_from_slice(&TPM_ALG_NULL.to_be_bytes());
            public_response[24..26].copy_from_slice(&TPM_ALG_NULL.to_be_bytes());
            public_response[26..28].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
            public_response[28..30].copy_from_slice(&2048_u16.to_be_bytes());
            public_response[30..34].copy_from_slice(&0_u32.to_be_bytes());
            public_response[34..36].copy_from_slice(&256_u16.to_be_bytes());
            public_response[36..292].fill(0xa5);
            public_response[291] = 1;
            public_response[292..294].copy_from_slice(&34_u16.to_be_bytes());
            let public_name_digest = Sha256::digest(&public_response[12..292]);
            public_response[294..296].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
            public_response[296..328].copy_from_slice(&public_name_digest);
            let Ok(parsed_public) = decode_attestation_public(&public_response) else {
                return false;
            };
            let AttestationPublicKey::Rsa(parsed_key) = parsed_public.key else {
                return false;
            };
            if parsed_key.modulus_length != 256 || parsed_key.exponent != 0 {
                return false;
            }
            if bind_attestation_public_name(&parsed_public, &parsed_public.name).is_err() {
                return false;
            }
            let mut wrong_name = parsed_public.name;
            wrong_name[33] ^= 1;
            if bind_attestation_public_name(&parsed_public, &wrong_name)
                != Err(Error::CredentialMismatch)
            {
                return false;
            }
            public_response[5] = 0;
            if decode_attestation_public(&public_response) != Err(Error::MalformedResponse) {
                return false;
            }
            let mut name_policy = [0_u8; ATTESTATION_NAME_POLICY_BYTES];
            name_policy[..4].copy_from_slice(&ATTESTATION_NAME_POLICY_MAGIC);
            name_policy[4..6].copy_from_slice(&ATTESTATION_NAME_POLICY_VERSION.to_be_bytes());
            name_policy[6..8].copy_from_slice(&(ATTESTATION_NAME_POLICY_SIGNED_BYTES as u16).to_be_bytes());
            name_policy[8..10].copy_from_slice(&TPM_ALG_SHA256.to_be_bytes());
            name_policy[10..].fill(0x22);
            name_policy[42..106].copy_from_slice(&[
                0x88, 0x4f, 0x4f, 0x52, 0xbc, 0x41, 0x4e, 0xf4, 0x02, 0x44, 0x56, 0x10, 0x3d, 0xa6, 0xa2, 0x95,
                0x17, 0xf3, 0x05, 0x99, 0x6e, 0xec, 0xeb, 0x8b, 0xa6, 0x63, 0xd0, 0x3a, 0x39, 0x15, 0x1d, 0x88,
                0x8e, 0x57, 0x3c, 0x23, 0x54, 0x77, 0x5e, 0x24, 0xa0, 0x72, 0x34, 0x44, 0xfd, 0x93, 0x70, 0x7b,
                0x66, 0xe8, 0xa5, 0xd3, 0x81, 0x5f, 0xfb, 0x81, 0xae, 0x08, 0xbb, 0xe8, 0x42, 0xab, 0x0e, 0x03,
            ]);
            let policy_key = [
                0xea, 0x4a, 0x6c, 0x63, 0xe2, 0x9c, 0x52, 0x0a, 0xbe, 0xf5, 0x50, 0x7b, 0x13, 0x2e, 0xc5, 0xf9,
                0x95, 0x47, 0x76, 0xae, 0xbe, 0xbe, 0x7b, 0x92, 0x42, 0x1e, 0xea, 0x69, 0x14, 0x46, 0xd2, 0x2c,
            ];
            let Ok(policy_name) = verify_signed_attestation_name_policy(&name_policy, &policy_key) else {
                return false;
            };
            if policy_name[..2] != TPM_ALG_SHA256.to_be_bytes() || policy_name[2..] != [0x22; 32] {
                return false;
            }
            if authorize_quote(QuoteAuthorizationRequest {
                name_policy: &name_policy,
                trusted_policy_key: &policy_key,
                public: &parsed_public,
                quote: &quote,
                signature: &[],
                expected_nonce: &quote_nonce,
                expected_pcr: 7,
                expected_digest: &expected_quote_digest,
            }) != Err(Error::CredentialMismatch)
            {
                return false;
            }
            name_policy[42] ^= 1;
            if verify_signed_attestation_name_policy(&name_policy, &policy_key)
                != Err(Error::PolicySignature)
            {
                return false;
            }
            read_response[10..14].copy_from_slice(&33_u32.to_be_bytes());
            decode_nv_read_policy_response(&read_response, &mut read_data)
                == Err(Error::MalformedResponse)
        }
}
