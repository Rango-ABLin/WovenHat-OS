//! Stage 15.3: bounded, single-directory WovenFS-to-VFS operation adapter.
//!
//! This is an opt-in building block, NOT a mounted filesystem or namespace
//! dispatcher. The caller must own the Volume exclusively, authenticate the
//! caller's VFS/capability authority, and supply a trusted directory inode.
//! Do not expose these functions directly as user syscalls.
use crate::{block::BlockDevice, vfs, wfs};

pub fn translate_error(error: wfs::Error) -> vfs::Error {
    match error {
        wfs::Error::NotFound => vfs::Error::NotFound,
        wfs::Error::Exists => vfs::Error::AlreadyExists,
        wfs::Error::InvalidName | wfs::Error::NotADirectory => vfs::Error::InvalidPath,
        wfs::Error::NoSpace | wfs::Error::TooLarge => vfs::Error::Full,
        wfs::Error::NotEmpty => vfs::Error::NotEmpty,
        wfs::Error::Device | wfs::Error::Corrupt => vfs::Error::Io,
    }
}

/// Only a single canonical filename is accepted. Directory traversal and
/// mount-prefix resolution must happen in the trusted VFS layer.
pub fn validate_component(name: &str) -> Result<&str, vfs::Error> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.len() > wfs::MAX_NAME
        || name.bytes().any(|b| b == b'/' || b == 0)
    {
        return Err(vfs::Error::InvalidPath);
    }
    Ok(name)
}

/// Trusted adapter: directory ownership and access rights are preconditions.
pub struct DirectoryAdapter<'a, D: BlockDevice> {
    volume: &'a mut wfs::Volume<D>,
    directory: u64,
}

impl<'a, D: BlockDevice> DirectoryAdapter<'a, D> {
    pub fn new(volume: &'a mut wfs::Volume<D>, directory: u64) -> Self {
        Self { volume, directory }
    }

    pub fn lookup(&mut self, name: &str) -> Result<u64, vfs::Error> {
        self.volume
            .lookup(self.directory, validate_component(name)?)
            .map_err(translate_error)
    }

    pub fn create(&mut self, name: &str, mode: u32, now: u64) -> Result<u64, vfs::Error> {
        self.volume
            .create(self.directory, validate_component(name)?, mode, now)
            .map_err(translate_error)
    }

    pub fn unlink(&mut self, name: &str, now: u64) -> Result<(), vfs::Error> {
        self.volume
            .unlink(self.directory, validate_component(name)?, now)
            .map_err(translate_error)
    }

    pub fn read_file(&mut self, inode: u64, out: &mut [u8]) -> Result<usize, vfs::Error> {
        self.volume.read_file(inode, out).map_err(translate_error)
    }

    pub fn write_file(&mut self, inode: u64, data: &[u8], now: u64) -> Result<(), vfs::Error> {
        self.volume.write_file(inode, data, now).map_err(translate_error)
    }
}
