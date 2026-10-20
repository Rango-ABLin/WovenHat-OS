use libwoven::package::{self, HEADER_LEN, MAGIC, VERSION};
use sha2::{Digest, Sha256};

const SIGNATURE_DOMAIN: &[u8] = b"WOVENHAT-WPKG-V1";
const TEST_SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec,
    0x2c, 0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03,
    0x1c, 0xae, 0x7f, 0x60,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationError {
    Malformed(package::Error),
    InvalidSignature,
    UntrustedKey,
}

pub fn verify(bytes: &[u8]) -> Result<package::Package<'_>, VerificationError> {
    let parsed = package::parse(bytes).map_err(VerificationError::Malformed)?;
    let mut signed = Sha256::new();
    signed.update(SIGNATURE_DOMAIN);
    signed.update(parsed.signed_prefix);
    signed.update(&bytes[HEADER_LEN..]);
    let digest: [u8; 32] = signed.finalize().into();
    crate::signed_app::verify(&digest, &crate::signed_app::TRUSTED_PUBLIC_KEY, &parsed.signature)
        .map_err(|error| match error {
            crate::signed_app::VerificationError::InvalidPublicKey => VerificationError::UntrustedKey,
            _ => VerificationError::InvalidSignature,
        })?;
    Ok(parsed)
}

pub fn self_test() -> bool {
    const MANIFEST: [u8; 27] = [
        4, b'd', b'e', b'm', b'o', 1, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 1, 4,
        b'c', b'o', b'r', b'e',
    ];
    let mut bytes = [0u8; HEADER_LEN + MANIFEST.len() + 3];
    bytes[..4].copy_from_slice(&MAGIC);
    bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
    bytes[6..8].copy_from_slice(&(HEADER_LEN as u16).to_le_bytes());
    bytes[8..12].copy_from_slice(&(MANIFEST.len() as u32).to_le_bytes());
    bytes[12..20].copy_from_slice(&3_u64.to_le_bytes());
    bytes[20..28].copy_from_slice(&0_u64.to_le_bytes());
    bytes[HEADER_LEN..HEADER_LEN + MANIFEST.len()].copy_from_slice(&MANIFEST);
    bytes[HEADER_LEN + MANIFEST.len()..].copy_from_slice(b"ELF");
    let Ok(parsed) = package::parse(&bytes) else {
        return false;
    };
    if parsed.manifest.name != b"demo"
        || parsed.manifest.dependencies[0] != b"core"
        || parsed.application != b"ELF"
        || !parsed.resources.is_empty()
    {
        return false;
    }
    bytes[0] ^= 1;
    package::parse(&bytes) == Err(package::Error::BadMagic)
}

pub fn verification_self_test() -> bool {
    const MANIFEST: [u8; 27] = [
        4, b'd', b'e', b'm', b'o', 1, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 1, 4,
        b'c', b'o', b'r', b'e',
    ];
    let mut bytes = [0u8; HEADER_LEN + MANIFEST.len() + 3];
    bytes[..4].copy_from_slice(&MAGIC);
    bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
    bytes[6..8].copy_from_slice(&(HEADER_LEN as u16).to_le_bytes());
    bytes[8..12].copy_from_slice(&(MANIFEST.len() as u32).to_le_bytes());
    bytes[12..20].copy_from_slice(&3_u64.to_le_bytes());
    bytes[20..28].copy_from_slice(&0_u64.to_le_bytes());
    bytes[HEADER_LEN..HEADER_LEN + MANIFEST.len()].copy_from_slice(&MANIFEST);
    bytes[HEADER_LEN + MANIFEST.len()..].copy_from_slice(b"ELF");
    let mut signed = Sha256::new();
    signed.update(SIGNATURE_DOMAIN);
    signed.update(&bytes[..28]);
    signed.update(&bytes[HEADER_LEN..]);
    let digest: [u8; 32] = signed.finalize().into();
    let key_pair = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(TEST_SEED));
    let signature = key_pair.sk.sign(digest, None);
    bytes[28..HEADER_LEN].copy_from_slice(signature.as_ref());
    if verify(&bytes).is_err() {
        return false;
    }
    bytes[HEADER_LEN + MANIFEST.len()] ^= 1;
    verify(&bytes) == Err(VerificationError::InvalidSignature)
}

pub fn store_self_test() -> bool {
    let mut dependencies = [&[][..]; libwoven::package::MAX_DEPENDENCIES];
    let core = b"core";
    dependencies[0] = core;
    let app = libwoven::package::Manifest {
        name: b"app",
        version: 1,
        capabilities: 2,
        dependencies,
        dependency_count: 1,
    };
    let core_manifest = libwoven::package::Manifest {
        name: core,
        version: 1,
        capabilities: 1,
        dependencies: [&[][..]; libwoven::package::MAX_DEPENDENCIES],
        dependency_count: 0,
    };
    let mut transaction = libwoven::store::Store::new(0x3).transaction();
    if transaction.install(&app, [2; 32]) != Err(libwoven::store::Error::MissingDependency)
        || transaction.install(&core_manifest, [1; 32]).is_err()
        || transaction.install(&app, [2; 32]).is_err()
        || transaction.remove(core).is_ok()
    {
        return false;
    }
    let committed = transaction.commit();
    committed.count() == 2
}
