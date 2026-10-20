//! Bounded WovenStore catalog transactions.
//!
//! This module owns package metadata and transaction semantics only. Callers
//! must authenticate the WPKG envelope before converting its manifest into an
//! install request. Persistent WovenFS publication remains above this core.

use crate::package::{MAX_DEPENDENCIES, MAX_NAME_BYTES, Manifest};

pub const MAX_INSTALLED: usize = 32;
pub const RECORD_MAGIC: [u8; 4] = *b"WSTC";
pub const RECORD_VERSION: u16 = 1;
const RECORD_HEADER: usize = 4 + 2 + 2 + 8 + 4;
const ENTRY_BYTES: usize = 1 + MAX_NAME_BYTES + 8 + 8 + 32 + 1 + MAX_DEPENDENCIES * (1 + MAX_NAME_BYTES);
pub const MAX_RECORD_BYTES: usize = RECORD_HEADER + MAX_INSTALLED * ENTRY_BYTES + 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Full,
    InvalidName,
    InvalidVersion,
    CapabilityDenied,
    MissingDependency,
    DependencyInUse,
    AlreadyAbsent,
    Outdated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordError {
    TooSmall,
    BadMagic,
    BadVersion,
    BadLength,
    BadChecksum,
    BadName,
    BadDependency,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: [u8; MAX_NAME_BYTES],
    pub name_len: u8,
    pub version: u64,
    pub capabilities: u64,
    pub application_digest: [u8; 32],
    dependencies: [[u8; MAX_NAME_BYTES]; MAX_DEPENDENCIES],
    dependency_lengths: [u8; MAX_DEPENDENCIES],
    dependency_count: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Store {
    entries: [Option<Entry>; MAX_INSTALLED],
    count: usize,
    allowed_capabilities: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transaction {
    original: Store,
    working: Store,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstallRequest<'a> {
    pub manifest: &'a Manifest<'a>,
    pub application_digest: [u8; 32],
}

impl Store {
    pub const fn new(allowed_capabilities: u64) -> Self {
        Self {
            entries: [None; MAX_INSTALLED],
            count: 0,
            allowed_capabilities,
        }
    }

    pub const fn count(&self) -> usize {
        self.count
    }

    /// Encode a fixed-layout, checksummed catalog record for durable storage.
    pub fn encode_record(&self, out: &mut [u8]) -> Result<usize, RecordError> {
        let length = RECORD_HEADER
            .checked_add(
                self.count
                    .checked_mul(ENTRY_BYTES)
                    .ok_or(RecordError::BadLength)?,
            )
            .and_then(|length| length.checked_add(4))
            .ok_or(RecordError::BadLength)?;
        if out.len() < length {
            return Err(RecordError::TooSmall);
        }
        let mut cursor = 0;
        out[cursor..cursor + 4].copy_from_slice(&RECORD_MAGIC);
        cursor += 4;
        out[cursor..cursor + 2].copy_from_slice(&RECORD_VERSION.to_le_bytes());
        cursor += 2;
        out[cursor..cursor + 2].copy_from_slice(&(self.count as u16).to_le_bytes());
        cursor += 2;
        out[cursor..cursor + 8].copy_from_slice(&self.allowed_capabilities.to_le_bytes());
        cursor += 8;
        out[cursor..cursor + 4].copy_from_slice(&(length as u32).to_le_bytes());
        cursor += 4;
        for entry in self.entries.iter().flatten() {
            out[cursor] = entry.name_len;
            cursor += 1;
            out[cursor..cursor + MAX_NAME_BYTES].copy_from_slice(&entry.name);
            cursor += MAX_NAME_BYTES;
            out[cursor..cursor + 8].copy_from_slice(&entry.version.to_le_bytes());
            cursor += 8;
            out[cursor..cursor + 8].copy_from_slice(&entry.capabilities.to_le_bytes());
            cursor += 8;
            out[cursor..cursor + 32].copy_from_slice(&entry.application_digest);
            cursor += 32;
            out[cursor] = entry.dependency_count;
            cursor += 1;
            for index in 0..MAX_DEPENDENCIES {
                out[cursor] = entry.dependency_lengths[index];
                cursor += 1;
                out[cursor..cursor + MAX_NAME_BYTES].copy_from_slice(&entry.dependencies[index]);
                cursor += MAX_NAME_BYTES;
            }
        }
        let value = checksum(&out[..cursor]);
        out[cursor..cursor + 4].copy_from_slice(&value.to_le_bytes());
        Ok(length)
    }

    /// Decode and validate a complete durable catalog record.
    pub fn decode_record(bytes: &[u8]) -> Result<Self, RecordError> {
        if bytes.len() < RECORD_HEADER + 4 {
            return Err(RecordError::TooSmall);
        }
        if bytes[..4] != RECORD_MAGIC {
            return Err(RecordError::BadMagic);
        }
        if u16::from_le_bytes(bytes[4..6].try_into().unwrap()) != RECORD_VERSION {
            return Err(RecordError::BadVersion);
        }
        let count = u16::from_le_bytes(bytes[6..8].try_into().unwrap()) as usize;
        if count > MAX_INSTALLED {
            return Err(RecordError::BadLength);
        }
        let allowed_capabilities = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let length = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
        let expected = RECORD_HEADER
            .checked_add(count.checked_mul(ENTRY_BYTES).ok_or(RecordError::BadLength)?)
            .and_then(|value| value.checked_add(4))
            .ok_or(RecordError::BadLength)?;
        if length != expected || bytes.len() != expected {
            return Err(RecordError::BadLength);
        }
        let stored_checksum = u32::from_le_bytes(bytes[expected - 4..].try_into().unwrap());
        if checksum(&bytes[..expected - 4]) != stored_checksum {
            return Err(RecordError::BadChecksum);
        }
        let mut store = Self::new(allowed_capabilities);
        let mut cursor = RECORD_HEADER;
        for _ in 0..count {
            let name_len = bytes[cursor] as usize;
            cursor += 1;
            if name_len == 0 || name_len > MAX_NAME_BYTES {
                return Err(RecordError::BadName);
            }
            let mut name = [0u8; MAX_NAME_BYTES];
            name.copy_from_slice(&bytes[cursor..cursor + MAX_NAME_BYTES]);
            if !valid_name(&name[..name_len]) {
                return Err(RecordError::BadName);
            }
            if store.find(&name[..name_len]).is_some() {
                return Err(RecordError::Duplicate);
            }
            cursor += MAX_NAME_BYTES;
            let version = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            if version == 0 {
                return Err(RecordError::BadLength);
            }
            let capabilities = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            let mut application_digest = [0u8; 32];
            application_digest.copy_from_slice(&bytes[cursor..cursor + 32]);
            cursor += 32;
            let dependency_count = bytes[cursor] as usize;
            cursor += 1;
            if dependency_count > MAX_DEPENDENCIES {
                return Err(RecordError::BadDependency);
            }
            let mut dependencies = [[0u8; MAX_NAME_BYTES]; MAX_DEPENDENCIES];
            let mut dependency_lengths = [0u8; MAX_DEPENDENCIES];
            for index in 0..MAX_DEPENDENCIES {
                let dependency_length = bytes[cursor] as usize;
                cursor += 1;
                dependencies[index].copy_from_slice(&bytes[cursor..cursor + MAX_NAME_BYTES]);
                cursor += MAX_NAME_BYTES;
                if index < dependency_count {
                    if dependency_length == 0
                        || dependency_length > MAX_NAME_BYTES
                        || !valid_name(&dependencies[index][..dependency_length])
                    {
                        return Err(RecordError::BadDependency);
                    }
                    dependency_lengths[index] = dependency_length as u8;
                } else if dependency_length != 0 {
                    return Err(RecordError::BadDependency);
                }
            }
            store.entries[store.count] = Some(Entry {
                name,
                name_len: name_len as u8,
                version,
                capabilities,
                application_digest,
                dependencies,
                dependency_lengths,
                dependency_count: dependency_count as u8,
            });
            store.count += 1;
        }
        Ok(store)
    }

    pub fn contains(&self, name: &[u8]) -> bool {
        self.find(name).is_some()
    }

    pub fn transaction(&self) -> Transaction {
        Transaction {
            original: *self,
            working: *self,
        }
    }

    fn find(&self, name: &[u8]) -> Option<usize> {
        self.entries.iter().position(|entry| {
            entry.is_some_and(|entry| {
                entry.name_len as usize == name.len() && entry.name[..name.len()] == *name
            })
        })
    }

    fn install(
        &mut self,
        manifest: &Manifest<'_>,
        application_digest: [u8; 32],
    ) -> Result<(), Error> {
        if manifest.name.is_empty() || manifest.name.len() > MAX_NAME_BYTES {
            return Err(Error::InvalidName);
        }
        if manifest.version == 0 {
            return Err(Error::InvalidVersion);
        }
        if manifest.capabilities & !self.allowed_capabilities != 0 {
            return Err(Error::CapabilityDenied);
        }
        for dependency in manifest.dependencies.iter().take(manifest.dependency_count) {
            if self.find(dependency).is_none() {
                return Err(Error::MissingDependency);
            }
        }
        let slot = if let Some(index) = self.find(manifest.name) {
            let current = self.entries[index].expect("find returned a populated slot");
            if manifest.version <= current.version {
                return Err(Error::Outdated);
            }
            index
        } else {
            let index = self
                .entries
                .iter()
                .position(Option::is_none)
                .ok_or(Error::Full)?;
            self.count += 1;
            index
        };
        let mut name = [0u8; MAX_NAME_BYTES];
        name[..manifest.name.len()].copy_from_slice(manifest.name);
        let mut dependencies = [[0u8; MAX_NAME_BYTES]; MAX_DEPENDENCIES];
        let mut dependency_lengths = [0u8; MAX_DEPENDENCIES];
        for (index, dependency) in manifest
            .dependencies
            .iter()
            .take(manifest.dependency_count)
            .enumerate()
        {
            dependencies[index][..dependency.len()].copy_from_slice(dependency);
            dependency_lengths[index] = dependency.len() as u8;
        }
        self.entries[slot] = Some(Entry {
            name,
            name_len: name_len(manifest.name),
            version: manifest.version,
            capabilities: manifest.capabilities,
            application_digest,
            dependencies,
            dependency_lengths,
            dependency_count: manifest.dependency_count as u8,
        });
        Ok(())
    }

    fn remove(&mut self, name: &[u8]) -> Result<(), Error> {
        let index = self.find(name).ok_or(Error::AlreadyAbsent)?;
        if self.entries.iter().flatten().any(|entry| {
            entry
                .dependencies
                .iter()
                .zip(entry.dependency_lengths)
                .take(entry.dependency_count as usize)
                .any(|(dependency, length)| dependency[..length as usize] == *name)
        }) {
            return Err(Error::DependencyInUse);
        }
        self.entries[index] = None;
        self.count -= 1;
        Ok(())
    }
}

impl Transaction {
    pub fn install(
        &mut self,
        manifest: &Manifest<'_>,
        application_digest: [u8; 32],
    ) -> Result<(), Error> {
        self.working.install(manifest, application_digest)
    }

    pub fn remove(&mut self, name: &[u8]) -> Result<(), Error> {
        self.working.remove(name)
    }

    /// Install a bounded batch in dependency order. The working catalog is
    /// unchanged if any request fails; the caller still chooses commit versus
    /// rollback after all requests succeed.
    pub fn install_batch(&mut self, requests: &[InstallRequest<'_>]) -> Result<(), Error> {
        if requests.len() > MAX_INSTALLED {
            return Err(Error::Full);
        }
        let mut pending = [false; MAX_INSTALLED];
        for slot in pending.iter_mut().take(requests.len()) {
            *slot = true;
        }
        let mut remaining = requests.len();
        while remaining != 0 {
            let mut progress = false;
            for index in 0..requests.len() {
                if !pending[index] {
                    continue;
                }
                match self.install(requests[index].manifest, requests[index].application_digest) {
                    Ok(()) => {
                        pending[index] = false;
                        remaining -= 1;
                        progress = true;
                    }
                    Err(Error::MissingDependency) => {}
                    Err(error) => return Err(error),
                }
            }
            if !progress {
                return Err(Error::MissingDependency);
            }
        }
        Ok(())
    }

    pub fn commit(self) -> Store {
        self.working
    }

    pub fn rollback(self) -> Store {
        self.original
    }
}

const fn name_len(name: &[u8]) -> u8 {
    name.len() as u8
}

fn valid_name(name: &[u8]) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && name.iter().all(|byte| {
            matches!(*byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-')
        })
}

fn checksum(bytes: &[u8]) -> u32 {
    let mut value = 0x811c9dc5u32;
    for byte in bytes {
        value ^= u32::from(*byte);
        value = value.wrapping_mul(0x01000193);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::{Error, Store};
    use crate::package::Manifest;

    fn manifest<'a>(
        name: &'a [u8],
        version: u64,
        capabilities: u64,
        dependency: Option<&'a [u8]>,
    ) -> Manifest<'a> {
        let mut dependencies = [&[][..]; crate::package::MAX_DEPENDENCIES];
        let dependency_count = if let Some(value) = dependency {
            dependencies[0] = value;
            1
        } else {
            0
        };
        Manifest {
            name,
            version,
            capabilities,
            dependencies,
            dependency_count,
        }
    }

    #[test]
    fn installs_updates_and_removes_with_dependency_protection() {
        let mut transaction = Store::new(0x3).transaction();
        transaction
            .install(&manifest(b"core", 1, 1, None), [1; 32])
            .unwrap();
        transaction
            .install(&manifest(b"app", 1, 2, Some(b"core")), [2; 32])
            .unwrap();
        assert_eq!(transaction.remove(b"core"), Err(Error::DependencyInUse));
        assert_eq!(
            transaction.install(&manifest(b"app", 1, 2, Some(b"core")), [3; 32]),
            Err(Error::Outdated)
        );
        transaction
            .install(&manifest(b"app", 2, 2, Some(b"core")), [4; 32])
            .unwrap();
        let store = transaction.commit();
        assert_eq!(store.count(), 2);
    }

    #[test]
    fn missing_dependencies_capabilities_and_rollback_are_bounded() {
        let store = Store::new(1);
        let mut transaction = store.transaction();
        assert_eq!(
            transaction.install(&manifest(b"app", 1, 1, Some(b"core")), [0; 32]),
            Err(Error::MissingDependency)
        );
        assert_eq!(
            transaction.install(&manifest(b"wide", 1, 2, None), [0; 32]),
            Err(Error::CapabilityDenied)
        );
        assert_eq!(transaction.rollback(), store);
    }

    #[test]
    fn batch_resolution_accepts_reverse_dependency_order_atomically() {
        let dependency = manifest(b"core", 1, 1, None);
        let application = manifest(b"app", 1, 1, Some(b"core"));
        let requests = [
            super::InstallRequest {
                manifest: &application,
                application_digest: [2; 32],
            },
            super::InstallRequest {
                manifest: &dependency,
                application_digest: [1; 32],
            },
        ];
        let store = Store::new(1);
        let mut transaction = store.transaction();
        transaction.install_batch(&requests).unwrap();
        assert_eq!(transaction.commit().count(), 2);
    }

    #[test]
    fn durable_record_round_trip_and_checksum_rejection() {
        let mut transaction = Store::new(0x3).transaction();
        transaction
            .install(&manifest(b"core", 1, 1, None), [1; 32])
            .unwrap();
        let store = transaction.commit();
        let mut bytes = [0u8; super::MAX_RECORD_BYTES];
        let length = store.encode_record(&mut bytes).unwrap();
        assert_eq!(Store::decode_record(&bytes[..length]), Ok(store));
        bytes[length - 1] ^= 1;
        assert_eq!(
            Store::decode_record(&bytes[..length]),
            Err(super::RecordError::BadChecksum)
        );
    }
}
