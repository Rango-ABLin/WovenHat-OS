//! Stage 15.4 signed-application verification boundary.
//!
//! The verifier is allocation-free at the call boundary and uses the pure
//! Rust, `no_std` Ed25519 implementation. The caller supplies the immutable
//! trust key; packaging, provisioning, and enforcement policy are layered on
//! top of this primitive.

use ed25519_compact::{PublicKey, Signature};

pub const PACKAGE_MAGIC: [u8; 4] = *b"WAPP";
pub const PACKAGE_VERSION: u16 = 1;
const HEADER_LEN: usize = 4 + 2 + 2 + 8 + 64;

/// Development application trust root used by tests when no provisioned key
/// is supplied. Release builds must provide
/// `WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX` at compile time.
#[cfg(any(not(feature = "system-apps"), feature = "qemu-test"))]
const TEST_TRUSTED_PUBLIC_KEY: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

const fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        b'A'..=b'F' => value - b'A' + 10,
        _ => panic!("invalid application trust public key"),
    }
}

const fn provisioned_key(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        panic!("application trust public key must be 64 hex digits");
    }
    let mut key = [0u8; 32];
    let mut index = 0;
    while index < 32 {
        key[index] = (hex_nibble(bytes[index * 2]) << 4) | hex_nibble(bytes[index * 2 + 1]);
        index += 1;
    }
    key
}

#[cfg(feature = "system-apps")]
pub const TRUSTED_PUBLIC_KEY: [u8; 32] = match option_env!("WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX") {
    Some(value) => provisioned_key(value),
    None => panic!("system-apps requires WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX"),
};

#[cfg(not(feature = "system-apps"))]
pub const TRUSTED_PUBLIC_KEY: [u8; 32] = match option_env!("WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX") {
    Some(value) => provisioned_key(value),
    None => TEST_TRUSTED_PUBLIC_KEY,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationError {
    InvalidPublicKey,
    InvalidSignature,
    MalformedPackage,
    UnsupportedVersion,
    UntrustedKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedImage<'a> {
    bytes: &'a [u8],
}

impl<'a> VerifiedImage<'a> {
    pub fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

pub fn verify<'a>(
    image: &'a [u8],
    public_key: &[u8; 32],
    signature: &[u8; 64],
) -> Result<VerifiedImage<'a>, VerificationError> {
    let key = PublicKey::from_slice(public_key).map_err(|_| VerificationError::InvalidPublicKey)?;
    let signature = Signature::new(*signature);
    key.verify(image, &signature)
        .map_err(|_| VerificationError::InvalidSignature)?;
    Ok(VerifiedImage { bytes: image })
}

/// Verify the versioned WAPP envelope and return the exact immutable ELF
/// payload. The envelope does not carry a user-selected trust key: the key is
/// the kernel's provisioned root, so copying a public key into a package does
/// not create authority.
pub fn verify_package(package: &[u8]) -> Result<VerifiedImage<'_>, VerificationError> {
    verify_package_with_key(package, &TRUSTED_PUBLIC_KEY)
}

fn verify_package_with_key<'a>(
    package: &'a [u8],
    public_key: &[u8; 32],
) -> Result<VerifiedImage<'a>, VerificationError> {
    if package.len() < HEADER_LEN || package[..4] != PACKAGE_MAGIC {
        return Err(VerificationError::MalformedPackage);
    }
    let version = u16::from_le_bytes([package[4], package[5]]);
    let header_len = u16::from_le_bytes([package[6], package[7]]) as usize;
    if version != PACKAGE_VERSION {
        return Err(VerificationError::UnsupportedVersion);
    }
    if header_len != HEADER_LEN || package.len() < header_len {
        return Err(VerificationError::MalformedPackage);
    }
    let image_len = u64::from_le_bytes([
        package[8],
        package[9],
        package[10],
        package[11],
        package[12],
        package[13],
        package[14],
        package[15],
    ]) as usize;
    let image_end = header_len
        .checked_add(image_len)
        .ok_or(VerificationError::MalformedPackage)?;
    if image_end != package.len() {
        return Err(VerificationError::MalformedPackage);
    }
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&package[16..80]);
    verify(&package[header_len..], public_key, &signature).map_err(|error| match error {
        VerificationError::InvalidPublicKey => VerificationError::UntrustedKey,
        other => other,
    })
}

#[cfg(feature = "qemu-test")]
pub fn self_test() -> bool {
    // RFC 8032 test vector 1, plus a one-byte tamper check.
    let public_key = TEST_TRUSTED_PUBLIC_KEY;
    let signature = [
        0xe5, 0x56, 0x43, 0x00, 0xc3, 0x60, 0xac, 0x72, 0x90, 0x86, 0xe2, 0xcc, 0x80, 0x6e, 0x82,
        0x8a, 0x84, 0x87, 0x7f, 0x1e, 0xb8, 0xe5, 0xd9, 0x74, 0xd8, 0x73, 0xe0, 0x65, 0x22, 0x49,
        0x01, 0x55, 0x5f, 0xb8, 0x82, 0x15, 0x90, 0xa3, 0x3b, 0xac, 0xc6, 0x1e, 0x39, 0x70, 0x1c,
        0xf9, 0xb4, 0x6b, 0xd2, 0x5b, 0xf5, 0xf0, 0x59, 0x5b, 0xbe, 0x24, 0x65, 0x51, 0x41, 0x43,
        0x8e, 0x7a, 0x10, 0x0b,
    ];
    let mut package = [0u8; HEADER_LEN];
    package[..4].copy_from_slice(&PACKAGE_MAGIC);
    package[4..6].copy_from_slice(&PACKAGE_VERSION.to_le_bytes());
    package[6..8].copy_from_slice(&(HEADER_LEN as u16).to_le_bytes());
    package[16..].copy_from_slice(&signature);
    verify(&[], &public_key, &signature).is_ok()
        && verify(&[0], &public_key, &signature).is_err()
        && verify_package_with_key(&package, &TEST_TRUSTED_PUBLIC_KEY).is_ok()
        && verify_package_with_key(&package[..package.len() - 1], &TEST_TRUSTED_PUBLIC_KEY).is_err()
}
