//! Bounded WovenPackages v1 envelope and manifest parser.
//!
//! The parser is allocation-free and does not authenticate a package by
//! itself. The kernel/package service must verify the returned signature over
//! a domain-separated SHA-256 digest of the immutable header prefix, manifest,
//! application and resource bytes with the provisioned trust root before
//! installation.

pub const MAGIC: [u8; 4] = *b"WPKG";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 4 + 2 + 2 + 4 + 8 + 8 + 64;
pub const MAX_MANIFEST_BYTES: usize = 4096;
pub const MAX_APPLICATION_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_RESOURCE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_NAME_BYTES: usize = 48;
pub const MAX_DEPENDENCIES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooShort,
    BadMagic,
    UnsupportedVersion,
    BadHeader,
    BadLength,
    BadManifest,
    BadName,
    TooManyDependencies,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manifest<'a> {
    pub name: &'a [u8],
    pub version: u64,
    pub capabilities: u64,
    pub dependencies: [&'a [u8]; MAX_DEPENDENCIES],
    pub dependency_count: usize,
}

impl<'a> Manifest<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > MAX_MANIFEST_BYTES {
            return Err(Error::BadManifest);
        }
        let mut cursor = 0;
        let name_len = *bytes.get(cursor).ok_or(Error::BadManifest)? as usize;
        cursor += 1;
        let name_end = cursor.checked_add(name_len).ok_or(Error::BadManifest)?;
        let name = bytes.get(cursor..name_end).ok_or(Error::BadManifest)?;
        validate_name(name)?;
        cursor = name_end;
        let version = read_u64(bytes, &mut cursor)?;
        if version == 0 {
            return Err(Error::BadManifest);
        }
        let capabilities = read_u64(bytes, &mut cursor)?;
        let dependency_count = *bytes.get(cursor).ok_or(Error::BadManifest)? as usize;
        cursor += 1;
        if dependency_count > MAX_DEPENDENCIES {
            return Err(Error::TooManyDependencies);
        }
        let mut dependencies = [&[][..]; MAX_DEPENDENCIES];
        for dependency in dependencies.iter_mut().take(dependency_count) {
            let length = *bytes.get(cursor).ok_or(Error::BadManifest)? as usize;
            cursor += 1;
            let end = cursor.checked_add(length).ok_or(Error::BadManifest)?;
            let value = bytes.get(cursor..end).ok_or(Error::BadManifest)?;
            validate_name(value)?;
            *dependency = value;
            cursor = end;
        }
        if cursor != bytes.len() {
            return Err(Error::BadManifest);
        }
        Ok(Self {
            name,
            version,
            capabilities,
            dependencies,
            dependency_count,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Package<'a> {
    pub manifest: Manifest<'a>,
    pub application: &'a [u8],
    pub resources: &'a [u8],
    pub signature: [u8; 64],
    /// Immutable bytes covered before the signature field.
    pub signed_prefix: &'a [u8],
}

pub fn parse(bytes: &[u8]) -> Result<Package<'_>, Error> {
    if bytes.len() < HEADER_LEN {
        return Err(Error::TooShort);
    }
    if bytes[..4] != MAGIC {
        return Err(Error::BadMagic);
    }
    if u16::from_le_bytes([bytes[4], bytes[5]]) != VERSION {
        return Err(Error::UnsupportedVersion);
    }
    if usize::from(u16::from_le_bytes([bytes[6], bytes[7]])) != HEADER_LEN {
        return Err(Error::BadHeader);
    }
    let manifest_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let application_len = u64::from_le_bytes(bytes[12..20].try_into().unwrap());
    let resources_len = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
    if manifest_len == 0
        || manifest_len > MAX_MANIFEST_BYTES
        || application_len == 0
        || application_len > MAX_APPLICATION_BYTES
        || resources_len > MAX_RESOURCE_BYTES
    {
        return Err(Error::BadLength);
    }
    let app_len = usize::try_from(application_len).map_err(|_| Error::BadLength)?;
    let resource_len = usize::try_from(resources_len).map_err(|_| Error::BadLength)?;
    let manifest_end = HEADER_LEN
        .checked_add(manifest_len)
        .ok_or(Error::BadLength)?;
    let app_end = manifest_end.checked_add(app_len).ok_or(Error::BadLength)?;
    let resource_end = app_end.checked_add(resource_len).ok_or(Error::BadLength)?;
    if resource_end != bytes.len() {
        return Err(Error::BadLength);
    }
    let manifest = Manifest::parse(&bytes[HEADER_LEN..manifest_end])?;
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&bytes[28..HEADER_LEN]);
    Ok(Package {
        manifest,
        application: &bytes[manifest_end..app_end],
        resources: &bytes[app_end..resource_end],
        signature,
        signed_prefix: &bytes[..28],
    })
}

fn validate_name(name: &[u8]) -> Result<(), Error> {
    if name.is_empty()
        || name.len() > MAX_NAME_BYTES
        || name.iter().any(
            |byte| !matches!(*byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-'),
        )
    {
        return Err(Error::BadName);
    }
    Ok(())
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, Error> {
    let end = cursor.checked_add(8).ok_or(Error::BadManifest)?;
    let value = u64::from_le_bytes(
        bytes
            .get(*cursor..end)
            .ok_or(Error::BadManifest)?
            .try_into()
            .unwrap(),
    );
    *cursor = end;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{Error, HEADER_LEN, MAGIC, VERSION, parse};

    fn valid_package() -> Vec<u8> {
        let manifest = [
            4, b'd', b'e', b'm', b'o', // name
            1, 0, 0, 0, 0, 0, 0, 0, // version
            5, 0, 0, 0, 0, 0, 0, 0, // capabilities
            1, // dependency count
            4, b'c', b'o', b'r', b'e',
        ];
        let application = b"ELF";
        let resources = b"resource";
        let mut package =
            vec![0u8; HEADER_LEN + manifest.len() + application.len() + resources.len()];
        package[..4].copy_from_slice(&MAGIC);
        package[4..6].copy_from_slice(&VERSION.to_le_bytes());
        package[6..8].copy_from_slice(&(HEADER_LEN as u16).to_le_bytes());
        package[8..12].copy_from_slice(&(manifest.len() as u32).to_le_bytes());
        package[12..20].copy_from_slice(&(application.len() as u64).to_le_bytes());
        package[20..28].copy_from_slice(&(resources.len() as u64).to_le_bytes());
        package[HEADER_LEN..HEADER_LEN + manifest.len()].copy_from_slice(&manifest);
        let app_start = HEADER_LEN + manifest.len();
        package[app_start..app_start + application.len()].copy_from_slice(application);
        package[app_start + application.len()..].copy_from_slice(resources);
        package
    }

    #[test]
    fn parses_bounded_manifest_and_payloads() {
        let package = valid_package();
        let parsed = parse(&package).unwrap();
        assert_eq!(parsed.manifest.name, b"demo");
        assert_eq!(parsed.manifest.version, 1);
        assert_eq!(parsed.manifest.capabilities, 5);
        assert_eq!(parsed.manifest.dependencies[0], b"core");
        assert_eq!(parsed.application, b"ELF");
        assert_eq!(parsed.resources, b"resource");
        assert_eq!(parsed.signed_prefix, &package[..28]);
    }

    #[test]
    fn rejects_truncation_bad_names_and_length_mismatch() {
        let package = valid_package();
        assert_eq!(parse(&package[..package.len() - 1]), Err(Error::BadLength));
        let mut bad_name = package.clone();
        bad_name[HEADER_LEN + 1] = b'/';
        assert_eq!(parse(&bad_name), Err(Error::BadName));
        let mut bad_length = package;
        bad_length[12..20].copy_from_slice(&999u64.to_le_bytes());
        assert_eq!(parse(&bad_length), Err(Error::BadLength));
    }
}
