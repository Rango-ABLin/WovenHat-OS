//! Capability-gated WovenPackage publication through native WovenFS.
//!
//! The caller must authenticate the WPKG envelope before calling `install`.
//! Payloads are versioned and chunked because native WovenFS files are bounded
//! to `MAX_FILE_SIZE`; the catalog is committed only after every chunk is
//! durable, and failed publication removes chunks written by that attempt.

use alloc::format;
use core::str;

use ed25519_compact::{KeyPair, Seed};
use libwoven::package::Package;
use sha2::{Digest, Sha256};

const CATALOG_PATH: &str = "/wfs/packages/catalog.wstc";
const APPLICATION_DIRECTORY: &str = "/wfs/packages/apps";
const RESOURCE_DIRECTORY: &str = "/wfs/packages/resources";
const MAX_CHUNKS: usize = (libwoven::package::MAX_APPLICATION_BYTES as usize).div_ceil(crate::wfs::MAX_FILE_SIZE);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidName,
    Storage,
    Catalog(libwoven::store::RecordError),
    Package(libwoven::store::Error),
    TooManyChunks,
}

fn package_name<'a>(package: &'a Package<'a>) -> Result<&'a str, Error> {
    str::from_utf8(package.manifest.name).map_err(|_| Error::InvalidName)
}

fn ensure_directory(path: &str, now: u64) -> Result<(), Error> {
    match crate::wfs::mkdir_vfs_path(path, now) {
        Ok(()) | Err(crate::wfs::Error::Exists) => Ok(()),
        Err(_) => Err(Error::Storage),
    }
}

fn chunk_path(directory: &str, name: &str, version: u64, index: usize) -> alloc::string::String {
    format!("{directory}/{name}-v{version}-{index:04}.bin")
}

fn remove_chunks(directory: &str, name: &str, version: u64, count: usize, now: u64) {
    for index in 0..count {
        let path = chunk_path(directory, name, version, index);
        let _ = crate::wfs::unlink_vfs_path(&path, now);
    }
}

fn write_chunks(
    authority: &crate::wfs::AuthorizedWfsWriter,
    directory: &str,
    name: &str,
    version: u64,
    bytes: &[u8],
    now: u64,
) -> Result<usize, Error> {
    if bytes.is_empty() {
        return Ok(0);
    }
    let count = bytes.len().div_ceil(crate::wfs::MAX_FILE_SIZE);
    if count > MAX_CHUNKS {
        return Err(Error::TooManyChunks);
    }
    for index in 0..count {
        let start = index * crate::wfs::MAX_FILE_SIZE;
        let end = (start + crate::wfs::MAX_FILE_SIZE).min(bytes.len());
        let path = chunk_path(directory, name, version, index);
        if crate::wfs::write_vfs_path_durable(authority, &path, &bytes[start..end], now).is_err() {
            remove_chunks(directory, name, version, index, now);
            return Err(Error::Storage);
        }
    }
    Ok(count)
}

/// Publish one already-authenticated package and return the new catalog.
pub fn install(
    authority: &crate::wfs::AuthorizedWfsWriter,
    current: &libwoven::store::Store,
    package: &Package<'_>,
    now: u64,
) -> Result<libwoven::store::Store, Error> {
    let name = package_name(package)?;
    ensure_directory("/wfs/packages", now)?;
    ensure_directory(APPLICATION_DIRECTORY, now)?;
    ensure_directory(RESOURCE_DIRECTORY, now)?;
    let mut transaction = current.transaction();
    let application_digest: [u8; 32] = Sha256::digest(package.application).into();
    transaction
        .install(&package.manifest, application_digest)
        .map_err(Error::Package)?;
    let application_chunks = write_chunks(
        authority,
        APPLICATION_DIRECTORY,
        name,
        package.manifest.version,
        package.application,
        now,
    )?;
    if write_chunks(
        authority,
        RESOURCE_DIRECTORY,
        name,
        package.manifest.version,
        package.resources,
        now,
    )
    .is_err()
    {
        remove_chunks(
            APPLICATION_DIRECTORY,
            name,
            package.manifest.version,
            application_chunks,
            now,
        );
        return Err(Error::Storage);
    }
    let updated = transaction.commit();
    let mut record = [0u8; libwoven::store::MAX_RECORD_BYTES];
    let length = updated.encode_record(&mut record).map_err(Error::Catalog)?;
    if crate::wfs::write_vfs_path_durable(authority, CATALOG_PATH, &record[..length], now).is_err() {
        remove_chunks(
            APPLICATION_DIRECTORY,
            name,
            package.manifest.version,
            application_chunks,
            now,
        );
        let resource_chunks = package.resources.len().div_ceil(crate::wfs::MAX_FILE_SIZE);
        remove_chunks(
            RESOURCE_DIRECTORY,
            name,
            package.manifest.version,
            resource_chunks,
            now,
        );
        return Err(Error::Storage);
    }
    Ok(updated)
}

#[cfg(feature = "stage21-7-test")]
pub fn self_test() -> bool {
    const MANIFEST: [u8; 22] = [
        4, b'c', b'o', b'r', b'e', 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let mut bytes = [0u8; libwoven::package::HEADER_LEN + MANIFEST.len() + 3];
    bytes[..4].copy_from_slice(&libwoven::package::MAGIC);
    bytes[4..6].copy_from_slice(&libwoven::package::VERSION.to_le_bytes());
    bytes[6..8].copy_from_slice(&(libwoven::package::HEADER_LEN as u16).to_le_bytes());
    bytes[8..12].copy_from_slice(&(MANIFEST.len() as u32).to_le_bytes());
    bytes[12..20].copy_from_slice(&3u64.to_le_bytes());
    bytes[20..28].copy_from_slice(&0u64.to_le_bytes());
    bytes[libwoven::package::HEADER_LEN..libwoven::package::HEADER_LEN + MANIFEST.len()]
        .copy_from_slice(&MANIFEST);
    bytes[libwoven::package::HEADER_LEN + MANIFEST.len()..].copy_from_slice(b"ELF");
    let mut signed = Sha256::new();
    signed.update(b"WOVENHAT-WPKG-V1");
    signed.update(&bytes[..28]);
    signed.update(&bytes[libwoven::package::HEADER_LEN..]);
    let digest: [u8; 32] = signed.finalize().into();
    let signature = KeyPair::from_seed(Seed::new([
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec,
        0x2c, 0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03,
        0x1c, 0xae, 0x7f, 0x60,
    ]))
    .sk
    .sign(digest, None);
    bytes[28..libwoven::package::HEADER_LEN].copy_from_slice(signature.as_ref());
    let Ok(package) = crate::package::verify(&bytes) else {
        return false;
    };
    if !crate::wfs::format_ata(64) {
        return false;
    }
    let Some(authority) = crate::update::UpdateState::authorized_writer() else {
        return false;
    };
    let current = libwoven::store::Store::new(1);
    let Ok(updated) = install(&authority, &current, &package, 1) else {
        return false;
    };
    let mut record = [0u8; libwoven::store::MAX_RECORD_BYTES];
    let Ok(length) = crate::wfs::read_vfs_path_durable(CATALOG_PATH, &mut record) else {
        return false;
    };
    if libwoven::store::Store::decode_record(&record[..length]) != Ok(updated) {
        return false;
    }
    let mut application = [0u8; crate::wfs::MAX_FILE_SIZE];
    crate::wfs::read_vfs_path_durable(
        "/wfs/packages/apps/core-v1-0000.bin",
        &mut application,
    ) == Ok(3)
        && application[..3] == *b"ELF"
}
