use libwoven::package::{self, HEADER_LEN, MAGIC, VERSION};

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
