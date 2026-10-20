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
const PREVIOUS_CATALOG_PATH: &str = "/wfs/packages/catalog.previous.wstc";
const JOURNAL_PATH: &str = "/wfs/packages/install.wpkj";
const ROLLBACK_PATH: &str = "/wfs/packages/rollback.wrbk";
const APPLICATION_DIRECTORY: &str = "/wfs/packages/apps";
const RESOURCE_DIRECTORY: &str = "/wfs/packages/resources";
const MAX_CHUNKS: usize = (libwoven::package::MAX_APPLICATION_BYTES as usize).div_ceil(crate::wfs::MAX_FILE_SIZE);
const JOURNAL_MAGIC: [u8; 4] = *b"WPKJ";
const JOURNAL_VERSION: u16 = 1;
const JOURNAL_INSTALL_PREPARED: u8 = 1;
const JOURNAL_INSTALL_COMMITTED: u8 = 2;
const JOURNAL_REMOVE_PREPARED: u8 = 3;
const JOURNAL_REMOVE_COMMITTED: u8 = 4;
const JOURNAL_BYTES: usize = 4 + 2 + 1 + 1 + libwoven::package::MAX_NAME_BYTES + 8 + 8 + 4 + 4 + 4 + 4 + 4;
const ROLLBACK_BYTES: usize = libwoven::store::MAX_RECORD_BYTES + 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InstallJournal {
    name: [u8; libwoven::package::MAX_NAME_BYTES],
    name_len: u8,
    version: u64,
    old_version: u64,
    application_chunks: u32,
    resource_chunks: u32,
    old_application_chunks: u32,
    old_resource_chunks: u32,
    state: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidName,
    Storage,
    Catalog(libwoven::store::RecordError),
    Package(libwoven::store::Error),
    SandboxDenied,
    TooManyChunks,
}

pub fn sandbox_for_entry(
    entry: &libwoven::store::Entry,
) -> Option<crate::wovenguard::SandboxProfile> {
    crate::wovenguard::package_sandbox(crate::capability::CapabilitySet::from_bits(
        entry.capabilities,
    ))
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

fn journal_checksum(bytes: &[u8]) -> u32 {
    let mut value = 0x811c9dc5u32;
    for byte in bytes {
        value ^= u32::from(*byte);
        value = value.wrapping_mul(0x01000193);
    }
    value
}

fn encode_journal(journal: InstallJournal) -> [u8; JOURNAL_BYTES] {
    let mut bytes = [0u8; JOURNAL_BYTES];
    let mut cursor = 0;
    bytes[cursor..cursor + 4].copy_from_slice(&JOURNAL_MAGIC);
    cursor += 4;
    bytes[cursor..cursor + 2].copy_from_slice(&JOURNAL_VERSION.to_le_bytes());
    cursor += 2;
    bytes[cursor] = journal.state;
    cursor += 1;
    bytes[cursor] = journal.name_len;
    cursor += 1;
    bytes[cursor..cursor + journal.name.len()].copy_from_slice(&journal.name);
    cursor += journal.name.len();
    bytes[cursor..cursor + 8].copy_from_slice(&journal.version.to_le_bytes());
    cursor += 8;
    bytes[cursor..cursor + 8].copy_from_slice(&journal.old_version.to_le_bytes());
    cursor += 8;
    bytes[cursor..cursor + 4].copy_from_slice(&journal.application_chunks.to_le_bytes());
    cursor += 4;
    bytes[cursor..cursor + 4].copy_from_slice(&journal.resource_chunks.to_le_bytes());
    cursor += 4;
    bytes[cursor..cursor + 4].copy_from_slice(&journal.old_application_chunks.to_le_bytes());
    cursor += 4;
    bytes[cursor..cursor + 4].copy_from_slice(&journal.old_resource_chunks.to_le_bytes());
    cursor += 4;
    let checksum = journal_checksum(&bytes[..cursor]);
    bytes[cursor..cursor + 4].copy_from_slice(&checksum.to_le_bytes());
    bytes
}

fn decode_journal(bytes: &[u8]) -> Option<InstallJournal> {
    if bytes.len() != JOURNAL_BYTES
        || bytes[..4] != JOURNAL_MAGIC
        || u16::from_le_bytes(bytes[4..6].try_into().ok()?) != JOURNAL_VERSION
        || journal_checksum(&bytes[..JOURNAL_BYTES - 4])
            != u32::from_le_bytes(bytes[JOURNAL_BYTES - 4..].try_into().ok()?)
    {
        return None;
    }
    let state = bytes[6];
    let name_len = bytes[7] as usize;
    if !matches!(
        state,
        JOURNAL_INSTALL_PREPARED
            | JOURNAL_INSTALL_COMMITTED
            | JOURNAL_REMOVE_PREPARED
            | JOURNAL_REMOVE_COMMITTED
    )
        || name_len == 0
        || name_len > libwoven::package::MAX_NAME_BYTES
    {
        return None;
    }
    let mut name = [0u8; libwoven::package::MAX_NAME_BYTES];
    let name_capacity = name.len();
    name.copy_from_slice(&bytes[8..8 + name_capacity]);
    let mut cursor = 8 + name_capacity;
    let version = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().ok()?);
    cursor += 8;
    let old_version = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().ok()?);
    cursor += 8;
    let application_chunks = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().ok()?);
    cursor += 4;
    let resource_chunks = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().ok()?);
    cursor += 4;
    let old_application_chunks = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().ok()?);
    cursor += 4;
    let old_resource_chunks = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().ok()?);
    Some(InstallJournal {
        name,
        name_len: name_len as u8,
        version,
        old_version,
        application_chunks,
        resource_chunks,
        old_application_chunks,
        old_resource_chunks,
        state,
    })
}

fn journal_name(journal: &InstallJournal) -> Result<&str, Error> {
    // Journal names are produced only from validated WPKG names. The fixed
    // storage record is still checked before any path is constructed.
    str::from_utf8(&journal.name[..journal.name_len as usize]).map_err(|_| Error::InvalidName)
}

fn write_journal(
    authority: &crate::wfs::AuthorizedWfsWriter,
    journal: InstallJournal,
    now: u64,
) -> Result<(), Error> {
    let bytes = encode_journal(journal);
    crate::wfs::write_vfs_path_durable(authority, JOURNAL_PATH, &bytes, now)
        .map_err(|_| Error::Storage)
}

fn clear_journal(authority: &crate::wfs::AuthorizedWfsWriter, now: u64) {
    let _ = crate::wfs::unlink_vfs_path(JOURNAL_PATH, now);
    let _ = authority;
}

fn encode_store(
    store: &libwoven::store::Store,
    out: &mut [u8; libwoven::store::MAX_RECORD_BYTES],
) -> Result<usize, Error> {
    store.encode_record(out).map_err(Error::Catalog)
}

fn write_catalog(
    authority: &crate::wfs::AuthorizedWfsWriter,
    path: &str,
    store: &libwoven::store::Store,
    now: u64,
) -> Result<(), Error> {
    let mut record = [0u8; libwoven::store::MAX_RECORD_BYTES];
    let length = encode_store(store, &mut record)?;
    crate::wfs::write_vfs_path_durable(authority, path, &record[..length], now)
        .map_err(|_| Error::Storage)
}

fn prepare_rollback(
    authority: &crate::wfs::AuthorizedWfsWriter,
    current: &libwoven::store::Store,
    now: u64,
) -> Result<(), Error> {
    let mut previous_record = [0u8; libwoven::store::MAX_RECORD_BYTES];
    match crate::wfs::read_vfs_path_durable(PREVIOUS_CATALOG_PATH, &mut previous_record) {
        Ok(length) => {
            let previous = libwoven::store::Store::decode_record(&previous_record[..length])
                .map_err(Error::Catalog)?;
            for index in 0..libwoven::store::MAX_INSTALLED {
                let Some(entry) = previous.entry_at(index) else {
                    continue;
                };
                let same_generation = current
                    .entry(&entry.name[..entry.name_len as usize])
                    .is_some_and(|current_entry| current_entry.version == entry.version);
                if !same_generation {
                    let name = str::from_utf8(&entry.name[..entry.name_len as usize])
                        .map_err(|_| Error::InvalidName)?;
                    remove_chunks(
                        APPLICATION_DIRECTORY,
                        name,
                        entry.version,
                        entry.application_chunks as usize,
                        now,
                    );
                    remove_chunks(
                        RESOURCE_DIRECTORY,
                        name,
                        entry.version,
                        entry.resource_chunks as usize,
                        now,
                    );
                }
            }
        }
        Err(crate::wfs::Error::NotFound) => {}
        Err(_) => return Err(Error::Storage),
    }
    write_catalog(authority, PREVIOUS_CATALOG_PATH, current, now)
}

fn write_rollback_intent(
    authority: &crate::wfs::AuthorizedWfsWriter,
    target: &libwoven::store::Store,
    now: u64,
) -> Result<(), Error> {
    let mut bytes = [0u8; ROLLBACK_BYTES];
    let length = encode_store(target, (&mut bytes[..libwoven::store::MAX_RECORD_BYTES]).try_into().unwrap())?;
    let checksum = journal_checksum(&bytes[..length]);
    bytes[length..length + 4].copy_from_slice(&checksum.to_le_bytes());
    crate::wfs::write_vfs_path_durable(authority, ROLLBACK_PATH, &bytes[..length + 4], now)
        .map_err(|_| Error::Storage)
}

fn read_rollback_intent(
    record: &mut [u8; libwoven::store::MAX_RECORD_BYTES],
) -> Result<libwoven::store::Store, Error> {
    let mut bytes = [0u8; ROLLBACK_BYTES];
    let length = crate::wfs::read_vfs_path_durable(ROLLBACK_PATH, &mut bytes)
        .map_err(|_| Error::Storage)?;
    if !(24..=ROLLBACK_BYTES).contains(&length) {
        return Err(Error::Storage);
    }
    let record_length = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    if record_length + 4 != length
        || journal_checksum(&bytes[..record_length])
            != u32::from_le_bytes(bytes[record_length..length].try_into().unwrap())
    {
        return Err(Error::Storage);
    }
    record[..record_length].copy_from_slice(&bytes[..record_length]);
    libwoven::store::Store::decode_record(&record[..record_length]).map_err(Error::Catalog)
}

fn clear_rollback(authority: &crate::wfs::AuthorizedWfsWriter, now: u64) {
    let _ = crate::wfs::unlink_vfs_path(ROLLBACK_PATH, now);
    let _ = authority;
}

fn restore_previous(
    authority: &crate::wfs::AuthorizedWfsWriter,
    current: &libwoven::store::Store,
    target: &libwoven::store::Store,
    now: u64,
) -> Result<(), Error> {
    write_catalog(authority, CATALOG_PATH, target, now)?;
    for index in 0..libwoven::store::MAX_INSTALLED {
        let Some(entry) = current.entry_at(index) else {
            continue;
        };
        let same_generation = target
            .entry(&entry.name[..entry.name_len as usize])
            .is_some_and(|target_entry| target_entry.version == entry.version);
        if !same_generation {
            let name = str::from_utf8(&entry.name[..entry.name_len as usize])
                .map_err(|_| Error::InvalidName)?;
            remove_chunks(
                APPLICATION_DIRECTORY,
                name,
                entry.version,
                entry.application_chunks as usize,
                now,
            );
            remove_chunks(
                RESOURCE_DIRECTORY,
                name,
                entry.version,
                entry.resource_chunks as usize,
                now,
            );
        }
    }
    Ok(())
}

/// Recover a prior install intent before accepting another package.
pub fn recover(
    authority: &crate::wfs::AuthorizedWfsWriter,
    current: &libwoven::store::Store,
    now: u64,
) -> Result<libwoven::store::Store, Error> {
    let mut rollback_record = [0u8; libwoven::store::MAX_RECORD_BYTES];
    match crate::wfs::read_vfs_path_durable(ROLLBACK_PATH, &mut [0u8; ROLLBACK_BYTES]) {
        Ok(_) => {
            let target = read_rollback_intent(&mut rollback_record)?;
            restore_previous(authority, current, &target, now)?;
            clear_rollback(authority, now);
        }
        Err(crate::wfs::Error::NotFound) => {}
        Err(_) => return Err(Error::Storage),
    }
    let mut bytes = [0u8; JOURNAL_BYTES];
    let length = match crate::wfs::read_vfs_path_durable(JOURNAL_PATH, &mut bytes) {
        Ok(length) => length,
        Err(crate::wfs::Error::NotFound) => return Ok(*current),
        Err(_) => return Err(Error::Storage),
    };
    let Some(journal) = decode_journal(&bytes[..length]) else {
        clear_journal(authority, now);
        return Err(Error::Storage);
    };
    let name = journal_name(&journal)?;
    let current_version = current.entry(name.as_bytes()).map(|entry| entry.version);
    let install_current = current_version == Some(journal.version);
    if matches!(journal.state, JOURNAL_INSTALL_PREPARED | JOURNAL_INSTALL_COMMITTED) {
        if install_current {
            remove_chunks(
                APPLICATION_DIRECTORY,
                name,
                journal.old_version,
                journal.old_application_chunks as usize,
                now,
            );
            remove_chunks(
                RESOURCE_DIRECTORY,
                name,
                journal.old_version,
                journal.old_resource_chunks as usize,
                now,
            );
        } else {
            remove_chunks(
                APPLICATION_DIRECTORY,
                name,
                journal.version,
                journal.application_chunks as usize,
                now,
            );
            remove_chunks(
                RESOURCE_DIRECTORY,
                name,
                journal.version,
                journal.resource_chunks as usize,
                now,
            );
        }
    } else if journal.state == JOURNAL_REMOVE_PREPARED && current_version == Some(journal.version) {
        // The catalog has not committed the removal yet; the old payload is
        // still authoritative and must survive recovery.
    } else {
        remove_chunks(
            APPLICATION_DIRECTORY,
            name,
            journal.version,
            journal.old_application_chunks as usize,
            now,
        );
        remove_chunks(
            RESOURCE_DIRECTORY,
            name,
            journal.version,
            journal.old_resource_chunks as usize,
            now,
        );
    }
    clear_journal(authority, now);
    Ok(*current)
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
    let current = recover(authority, current, now)?;
    let name = package_name(package)?;
    if crate::wovenguard::package_sandbox(crate::capability::CapabilitySet::from_bits(
        package.manifest.capabilities,
    ))
    .is_none()
    {
        return Err(Error::SandboxDenied);
    }
    ensure_directory("/wfs/packages", now)?;
    ensure_directory(APPLICATION_DIRECTORY, now)?;
    ensure_directory(RESOURCE_DIRECTORY, now)?;
    prepare_rollback(authority, &current, now)?;
    let mut transaction = current.transaction();
    let application_digest: [u8; 32] = Sha256::digest(package.application).into();
    let application_chunks = package
        .application
        .len()
        .div_ceil(crate::wfs::MAX_FILE_SIZE);
    let resource_chunks = package.resources.len().div_ceil(crate::wfs::MAX_FILE_SIZE);
    transaction
        .install_with_sizes(
            &package.manifest,
            application_digest,
            application_chunks as u32,
            resource_chunks as u32,
        )
        .map_err(Error::Package)?;
    if application_chunks > MAX_CHUNKS || resource_chunks > MAX_CHUNKS {
        return Err(Error::TooManyChunks);
    }
    let mut journal_name_bytes = [0u8; libwoven::package::MAX_NAME_BYTES];
    journal_name_bytes[..name.len()].copy_from_slice(name.as_bytes());
    let journal = InstallJournal {
        name: journal_name_bytes,
        name_len: name.len() as u8,
        version: package.manifest.version,
        application_chunks: application_chunks as u32,
        resource_chunks: resource_chunks as u32,
        state: JOURNAL_INSTALL_PREPARED,
        old_version: current.entry(name.as_bytes()).map_or(0, |entry| entry.version),
        old_application_chunks: current
            .entry(name.as_bytes())
            .map_or(0, |entry| entry.application_chunks),
        old_resource_chunks: current
            .entry(name.as_bytes())
            .map_or(0, |entry| entry.resource_chunks),
    };
    // The intent must be durable before payload publication begins.
    if write_journal(authority, journal, now).is_err() {
        remove_chunks(APPLICATION_DIRECTORY, name, package.manifest.version, application_chunks, now);
        remove_chunks(RESOURCE_DIRECTORY, name, package.manifest.version, resource_chunks, now);
        return Err(Error::Storage);
    }
    if write_chunks(
        authority,
        APPLICATION_DIRECTORY,
        name,
        package.manifest.version,
        package.application,
        now,
    )
    .is_err()
    {
        clear_journal(authority, now);
        return Err(Error::Storage);
    }
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
        clear_journal(authority, now);
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
        remove_chunks(
            RESOURCE_DIRECTORY,
            name,
            package.manifest.version,
            resource_chunks,
            now,
        );
        return Err(Error::Storage);
    }
    let committed = InstallJournal {
        state: JOURNAL_INSTALL_COMMITTED,
        ..journal
    };
    if write_journal(authority, committed, now).is_err() {
        return Err(Error::Storage);
    }
    clear_journal(authority, now);
    Ok(updated)
}

/// Remove one installed package with a catalog-first, crash-recoverable commit.
pub fn remove(
    authority: &crate::wfs::AuthorizedWfsWriter,
    current: &libwoven::store::Store,
    name: &[u8],
    now: u64,
) -> Result<libwoven::store::Store, Error> {
    let current = recover(authority, current, now)?;
    let entry = current.entry(name).ok_or(Error::Package(libwoven::store::Error::AlreadyAbsent))?;
    let name = str::from_utf8(name).map_err(|_| Error::InvalidName)?;
    prepare_rollback(authority, &current, now)?;
    let mut transaction = current.transaction();
    transaction.remove(name.as_bytes()).map_err(Error::Package)?;
    let mut journal_name = [0u8; libwoven::package::MAX_NAME_BYTES];
    journal_name[..name.len()].copy_from_slice(name.as_bytes());
    let journal = InstallJournal {
        name: journal_name,
        name_len: name.len() as u8,
        version: entry.version,
        old_version: entry.version,
        application_chunks: 0,
        resource_chunks: 0,
        old_application_chunks: entry.application_chunks,
        old_resource_chunks: entry.resource_chunks,
        state: JOURNAL_REMOVE_PREPARED,
    };
    write_journal(authority, journal, now)?;
    let updated = transaction.commit();
    let mut record = [0u8; libwoven::store::MAX_RECORD_BYTES];
    let length = updated.encode_record(&mut record).map_err(Error::Catalog)?;
    crate::wfs::write_vfs_path_durable(authority, CATALOG_PATH, &record[..length], now)
        .map_err(|_| Error::Storage)?;
    write_journal(
        authority,
        InstallJournal { state: JOURNAL_REMOVE_COMMITTED, ..journal },
        now,
    )?;
    clear_journal(authority, now);
    Ok(updated)
}

/// Restore the most recent durable package catalog generation.
pub fn rollback(
    authority: &crate::wfs::AuthorizedWfsWriter,
    current: &libwoven::store::Store,
    now: u64,
) -> Result<libwoven::store::Store, Error> {
    let current = recover(authority, current, now)?;
    let mut previous_record = [0u8; libwoven::store::MAX_RECORD_BYTES];
    let length = crate::wfs::read_vfs_path_durable(PREVIOUS_CATALOG_PATH, &mut previous_record)
        .map_err(|_| Error::Storage)?;
    let target = libwoven::store::Store::decode_record(&previous_record[..length])
        .map_err(Error::Catalog)?;
    write_rollback_intent(authority, &target, now)?;
    restore_previous(authority, &current, &target, now)?;
    clear_rollback(authority, now);
    Ok(target)
}

#[cfg(any(
    feature = "stage21-7-test",
    feature = "stage21-9-test",
    feature = "stage21-10-test"
    , feature = "stage21-11-test"
))]
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

#[cfg(any(
    feature = "stage21-8-test",
    feature = "stage21-9-test",
    feature = "stage21-10-test"
    , feature = "stage21-11-test"
))]
pub fn recovery_self_test() -> bool {
    if !crate::wfs::format_ata(64) {
        return false;
    }
    let Some(authority) = crate::update::UpdateState::authorized_writer() else {
        return false;
    };
    if ensure_directory("/wfs/packages", 1).is_err()
        || ensure_directory(APPLICATION_DIRECTORY, 1).is_err()
        || ensure_directory(RESOURCE_DIRECTORY, 1).is_err()
    {
        return false;
    }
    let payload = b"orphan";
    let path = "/wfs/packages/apps/ghost-v1-0000.bin";
    if crate::wfs::write_vfs_path_durable(&authority, path, payload, 1).is_err() {
        return false;
    }
    let mut name = [0u8; libwoven::package::MAX_NAME_BYTES];
    name[..5].copy_from_slice(b"ghost");
    let journal = InstallJournal {
        name,
        name_len: 5,
        version: 1,
        old_version: 0,
        application_chunks: 1,
        resource_chunks: 0,
        old_application_chunks: 0,
        old_resource_chunks: 0,
        state: JOURNAL_INSTALL_PREPARED,
    };
    if write_journal(&authority, journal, 1).is_err() {
        return false;
    }
    if recover(&authority, &libwoven::store::Store::new(1), 2).is_err() {
        return false;
    }
    let mut out = [0u8; crate::wfs::MAX_FILE_SIZE];
    matches!(
        crate::wfs::read_vfs_path_durable(path, &mut out),
        Err(crate::wfs::Error::NotFound)
    )
}

#[cfg(any(
    feature = "stage21-9-test",
    feature = "stage21-10-test",
    feature = "stage21-11-test"
))]
pub fn update_remove_self_test() -> bool {
    if !self_test() {
        return false;
    }
    let Some(authority) = crate::update::UpdateState::authorized_writer() else {
        return false;
    };
    let mut record = [0u8; libwoven::store::MAX_RECORD_BYTES];
    let Ok(length) = crate::wfs::read_vfs_path_durable(CATALOG_PATH, &mut record) else {
        return false;
    };
    let Ok(current) = libwoven::store::Store::decode_record(&record[..length]) else {
        return false;
    };
    let Ok(updated) = remove(&authority, &current, b"core", 2) else {
        return false;
    };
    if updated.contains(b"core") || rollback(&authority, &updated, 3).is_err() {
        return false;
    }
    let mut application = [0u8; crate::wfs::MAX_FILE_SIZE];
    crate::wfs::read_vfs_path_durable(
        "/wfs/packages/apps/core-v1-0000.bin",
        &mut application,
    ) == Ok(3)
        && application[..3] == *b"ELF"
}

#[cfg(feature = "stage21-10-test")]
pub fn rollback_self_test() -> bool {
    update_remove_self_test()
}

#[cfg(feature = "stage21-11-test")]
pub fn sandbox_self_test() -> bool {
    if !self_test() {
        return false;
    }
    let mut record = [0u8; libwoven::store::MAX_RECORD_BYTES];
    let Ok(length) = crate::wfs::read_vfs_path_durable(CATALOG_PATH, &mut record) else {
        return false;
    };
    let Ok(current) = libwoven::store::Store::decode_record(&record[..length]) else {
        return false;
    };
    let Some(entry) = current.entry(b"core") else {
        return false;
    };
    let Some(profile) = sandbox_for_entry(&entry) else {
        return false;
    };
    profile.allows(crate::capability::Capability::Console)
        && !profile.allows(crate::capability::Capability::FileRead)
        && profile.allows_service_discover(crate::wovenguard::ServiceClass::Application)
        && !profile.allows_service_discover(crate::wovenguard::ServiceClass::Storage)
        && crate::wovenguard::package_sandbox(
            crate::capability::CapabilitySet::only(crate::capability::Capability::InterruptControl),
        )
        .is_none()
}
