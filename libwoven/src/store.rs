//! Bounded WovenStore catalog transactions.
//!
//! This module owns package metadata and transaction semantics only. Callers
//! must authenticate the WPKG envelope before converting its manifest into an
//! install request. Persistent WovenFS publication remains above this core.

use crate::package::{Manifest, MAX_DEPENDENCIES, MAX_NAME_BYTES};

pub const MAX_INSTALLED: usize = 32;

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
            entry.is_some_and(|entry| entry.name_len as usize == name.len() && entry.name[..name.len()] == *name)
        })
    }

    fn install(&mut self, manifest: &Manifest<'_>, application_digest: [u8; 32]) -> Result<(), Error> {
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
            let index = self.entries.iter().position(Option::is_none).ok_or(Error::Full)?;
            self.count += 1;
            index
        };
        let mut name = [0u8; MAX_NAME_BYTES];
        name[..manifest.name.len()].copy_from_slice(manifest.name);
        let mut dependencies = [[0u8; MAX_NAME_BYTES]; MAX_DEPENDENCIES];
        let mut dependency_lengths = [0u8; MAX_DEPENDENCIES];
        for (index, dependency) in manifest.dependencies.iter().take(manifest.dependency_count).enumerate() {
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
            entry.dependencies.iter().zip(entry.dependency_lengths).take(entry.dependency_count as usize).any(|(dependency, length)| dependency[..length as usize] == *name)
        }) {
            return Err(Error::DependencyInUse);
        }
        self.entries[index] = None;
        self.count -= 1;
        Ok(())
    }
}

impl Transaction {
    pub fn install(&mut self, manifest: &Manifest<'_>, application_digest: [u8; 32]) -> Result<(), Error> {
        self.working.install(manifest, application_digest)
    }

    pub fn remove(&mut self, name: &[u8]) -> Result<(), Error> {
        self.working.remove(name)
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

#[cfg(test)]
mod tests {
    use super::{Error, Store};
    use crate::package::Manifest;

    fn manifest<'a>(name: &'a [u8], version: u64, capabilities: u64, dependency: Option<&'a [u8]>) -> Manifest<'a> {
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
        transaction.install(&manifest(b"core", 1, 1, None), [1; 32]).unwrap();
        transaction.install(&manifest(b"app", 1, 2, Some(b"core")), [2; 32]).unwrap();
        assert_eq!(transaction.remove(b"core"), Err(Error::DependencyInUse));
        assert_eq!(transaction.install(&manifest(b"app", 1, 2, Some(b"core")), [3; 32]), Err(Error::Outdated));
        transaction.install(&manifest(b"app", 2, 2, Some(b"core")), [4; 32]).unwrap();
        let store = transaction.commit();
        assert_eq!(store.count(), 2);
    }

    #[test]
    fn missing_dependencies_capabilities_and_rollback_are_bounded() {
        let store = Store::new(1);
        let mut transaction = store.transaction();
        assert_eq!(transaction.install(&manifest(b"app", 1, 1, Some(b"core")), [0; 32]), Err(Error::MissingDependency));
        assert_eq!(transaction.install(&manifest(b"wide", 1, 2, None), [0; 32]), Err(Error::CapabilityDenied));
        assert_eq!(transaction.rollback(), store);
    }
}
