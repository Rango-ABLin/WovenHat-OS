//! Bounded per-task secret handles for the WovenVault ABI.
//!
//! Secrets are copied into fixed kernel-owned slots, never returned through a
//! raw kernel pointer, and wiped on revoke or owner teardown. This is the
//! volatile handle layer; persistent encrypted records and TPM-backed key
//! provisioning remain separate platform work.

use crate::irq_lock::IrqMutex;

pub const MAX_SECRETS: usize = 16;
pub const MAX_SECRET_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecretHandle(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    InvalidHandle,
    NotOwner,
    TooLarge,
    Empty,
}

#[derive(Clone, Copy)]
struct Slot {
    generation: u32,
    owner: u64,
    length: usize,
    bytes: [u8; MAX_SECRET_BYTES],
}

impl Slot {
    const EMPTY: Self = Self {
        generation: 0,
        owner: 0,
        length: 0,
        bytes: [0; MAX_SECRET_BYTES],
    };

    fn active(&self) -> bool {
        self.generation != 0
    }

    fn wipe(&mut self) {
        for byte in &mut self.bytes {
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        self.generation = 0;
        self.owner = 0;
        self.length = 0;
    }
}

struct Vault {
    slots: [Slot; MAX_SECRETS],
    next_generation: u32,
}

impl Vault {
    const fn new() -> Self {
        Self {
            slots: [Slot::EMPTY; MAX_SECRETS],
            next_generation: 1,
        }
    }

    fn decode(handle: SecretHandle) -> Option<(usize, u32)> {
        let slot = (handle.0 & 0xff) as usize;
        let generation = (handle.0 >> 8) as u32;
        (slot < MAX_SECRETS && generation != 0).then_some((slot, generation))
    }

    fn store(&mut self, owner: u64, secret: &[u8]) -> Result<SecretHandle, Error> {
        if secret.is_empty() {
            return Err(Error::Empty);
        }
        if secret.len() > MAX_SECRET_BYTES {
            return Err(Error::TooLarge);
        }
        let slot = self
            .slots
            .iter()
            .position(|entry| !entry.active())
            .ok_or(Error::Capacity)?;
        let mut generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        if generation == 0 {
            generation = 1;
            self.next_generation = 2;
        }
        let entry = &mut self.slots[slot];
        entry.bytes[..secret.len()].copy_from_slice(secret);
        entry.length = secret.len();
        entry.owner = owner;
        entry.generation = generation;
        Ok(SecretHandle((u64::from(generation) << 8) | slot as u64))
    }

    fn read(&self, owner: u64, handle: SecretHandle, output: &mut [u8]) -> Result<usize, Error> {
        let (slot, generation) = Self::decode(handle).ok_or(Error::InvalidHandle)?;
        let entry = &self.slots[slot];
        if !entry.active() || entry.generation != generation {
            return Err(Error::InvalidHandle);
        }
        if entry.owner != owner {
            return Err(Error::NotOwner);
        }
        if output.len() < entry.length {
            return Err(Error::TooLarge);
        }
        output[..entry.length].copy_from_slice(&entry.bytes[..entry.length]);
        Ok(entry.length)
    }

    fn revoke(&mut self, owner: u64, handle: SecretHandle) -> Result<(), Error> {
        let (slot, generation) = Self::decode(handle).ok_or(Error::InvalidHandle)?;
        let entry = &mut self.slots[slot];
        if !entry.active() || entry.generation != generation {
            return Err(Error::InvalidHandle);
        }
        if entry.owner != owner {
            return Err(Error::NotOwner);
        }
        entry.wipe();
        Ok(())
    }

    fn release_owner(&mut self, owner: u64) {
        for entry in &mut self.slots {
            if entry.active() && entry.owner == owner {
                entry.wipe();
            }
        }
    }
}

static VAULT: IrqMutex<Vault> = IrqMutex::with_rank(Vault::new(), 20);

pub fn store(owner: u64, secret: &[u8]) -> Result<SecretHandle, Error> {
    VAULT.lock().store(owner, secret)
}

pub fn read(owner: u64, handle: SecretHandle, output: &mut [u8]) -> Result<usize, Error> {
    VAULT.lock().read(owner, handle, output)
}

pub fn revoke(owner: u64, handle: SecretHandle) -> Result<(), Error> {
    VAULT.lock().revoke(owner, handle)
}

pub fn release_owner(owner: u64) {
    VAULT.lock().release_owner(owner);
}

#[cfg(feature = "qemu-test")]
pub fn self_test() -> bool {
    let owner = 0x51e7;
    let other = 0x51e8;
    let value = *b"secret-value";
    let Ok(handle) = store(owner, &value) else {
        return false;
    };
    let mut output = [0u8; MAX_SECRET_BYTES];
    if read(other, handle, &mut output).is_ok()
        || read(owner, handle, &mut output) != Ok(value.len())
        || output[..value.len()] != value
        || revoke(other, handle).is_ok()
        || revoke(owner, handle).is_err()
        || read(owner, handle, &mut output).is_ok()
    {
        return false;
    }
    let Ok(reused) = store(owner, &value) else {
        return false;
    };
    reused != handle && revoke(owner, reused).is_ok()
}
