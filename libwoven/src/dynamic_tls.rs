//! Bounded dynamic-TLS template and per-thread storage foundation.

pub const MAX_MODULES: usize = 8;
pub const MAX_TLS_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleId {
    slot: u8,
    generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    Invalid,
    Stale,
    Misaligned,
    TooLarge,
    BufferTooSmall,
}

#[derive(Clone, Copy)]
struct Module {
    generation: u32,
    live: bool,
    memory_size: usize,
    alignment: usize,
    init: [u8; MAX_TLS_BYTES],
    init_size: usize,
}
impl Module {
    const EMPTY: Self = Self {
        generation: 1,
        live: false,
        memory_size: 0,
        alignment: 1,
        init: [0; MAX_TLS_BYTES],
        init_size: 0,
    };
}

pub struct Registry {
    modules: [Module; MAX_MODULES],
}
impl Registry {
    pub const fn new() -> Self {
        Self {
            modules: [Module::EMPTY; MAX_MODULES],
        }
    }
    pub fn register(
        &mut self,
        initializer: &[u8],
        memory_size: usize,
        alignment: usize,
    ) -> Result<ModuleId, Error> {
        if initializer.len() > memory_size || memory_size > MAX_TLS_BYTES {
            return Err(Error::TooLarge);
        }
        if !alignment.is_power_of_two() || alignment > MAX_TLS_BYTES {
            return Err(Error::Misaligned);
        }
        let (slot, module) = self
            .modules
            .iter_mut()
            .enumerate()
            .find(|(_, m)| !m.live)
            .ok_or(Error::Capacity)?;
        module.live = true;
        module.memory_size = memory_size;
        module.alignment = alignment;
        module.init_size = initializer.len();
        module.init[..initializer.len()].copy_from_slice(initializer);
        Ok(ModuleId {
            slot: slot as u8,
            generation: module.generation,
        })
    }
    pub fn unregister(&mut self, id: ModuleId) -> Result<(), Error> {
        let module = self
            .modules
            .get_mut(id.slot as usize)
            .ok_or(Error::Invalid)?;
        if !module.live || module.generation != id.generation {
            return Err(Error::Stale);
        }
        module.live = false;
        module.generation = module.generation.saturating_add(1).max(1);
        module.memory_size = 0;
        module.init_size = 0;
        Ok(())
    }
    pub fn initialize(&self, id: ModuleId, output: &mut [u8]) -> Result<usize, Error> {
        let module = self.modules.get(id.slot as usize).ok_or(Error::Invalid)?;
        if !module.live || module.generation != id.generation {
            return Err(Error::Stale);
        }
        if output.len() < module.memory_size {
            return Err(Error::BufferTooSmall);
        }
        output[..module.memory_size].fill(0);
        output[..module.init_size].copy_from_slice(&module.init[..module.init_size]);
        Ok(module.memory_size)
    }
    pub fn alignment(&self, id: ModuleId) -> Result<usize, Error> {
        let module = self.modules.get(id.slot as usize).ok_or(Error::Invalid)?;
        (module.live && module.generation == id.generation)
            .then_some(module.alignment)
            .ok_or(Error::Stale)
    }

    fn memory_size(&self, id: ModuleId) -> Result<usize, Error> {
        let module = self.modules.get(id.slot as usize).ok_or(Error::Invalid)?;
        (module.live && module.generation == id.generation)
            .then_some(module.memory_size)
            .ok_or(Error::Stale)
    }
}
impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

/// A bounded per-thread dynamic TLS vector.
///
/// Each published module owns one fixed storage block.  The generation in the
/// published handle prevents a reused module slot from exposing an old block.
/// This is the safe runtime contract that a future kernel-backed
/// `__tls_get_addr` adapter can call; it intentionally returns a reference into
/// caller-owned storage rather than retaining an unchecked raw pointer.
#[derive(Clone, Copy)]
struct DtvEntry {
    id: Option<ModuleId>,
    storage: [u8; MAX_TLS_BYTES],
}

impl DtvEntry {
    const EMPTY: Self = Self {
        id: None,
        storage: [0; MAX_TLS_BYTES],
    };
}

pub struct Dtv {
    entries: [DtvEntry; MAX_MODULES],
}

impl Dtv {
    pub const fn new() -> Self {
        Self {
            entries: [DtvEntry::EMPTY; MAX_MODULES],
        }
    }

    /// Publish and initialize a module in this thread's DTV.
    pub fn publish(&mut self, registry: &Registry, id: ModuleId) -> Result<(), Error> {
        let slot = id.slot as usize;
        let size = registry.memory_size(id)?;
        let entry = self.entries.get_mut(slot).ok_or(Error::Invalid)?;
        if entry.id != Some(id) {
            registry.initialize(id, &mut entry.storage)?;
            entry.id = Some(id);
        }
        debug_assert!(size <= entry.storage.len());
        Ok(())
    }

    /// Return the byte at a module-relative TLS offset, publishing lazily.
    pub fn get_addr(
        &mut self,
        registry: &Registry,
        id: ModuleId,
        offset: usize,
    ) -> Result<&mut u8, Error> {
        self.publish(registry, id)?;
        let size = registry.memory_size(id)?;
        if offset >= size {
            return Err(Error::Invalid);
        }
        self.entries[id.slot as usize]
            .storage
            .get_mut(offset)
            .ok_or(Error::Invalid)
    }

    /// Return the initialized module block for inspection or runtime adapters.
    pub fn block(&mut self, registry: &Registry, id: ModuleId) -> Result<&mut [u8], Error> {
        self.publish(registry, id)?;
        let size = registry.memory_size(id)?;
        Ok(&mut self.entries[id.slot as usize].storage[..size])
    }
}

impl Default for Dtv {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Dtv, Error, Registry};
    #[test]
    fn initializes_and_zero_fills_each_thread_block() {
        let mut registry = Registry::new();
        let id = registry.register(b"tls", 8, 8).unwrap();
        let mut output = [0xaa; 8];
        assert_eq!(registry.initialize(id, &mut output), Ok(8));
        assert_eq!(&output, b"tls\0\0\0\0\0");
        assert_eq!(registry.alignment(id), Ok(8));
    }
    #[test]
    fn rejects_invalid_templates_and_stale_generations() {
        let mut registry = Registry::new();
        assert_eq!(registry.register(b"long", 2, 4), Err(Error::TooLarge));
        assert_eq!(registry.register(b"x", 2, 3), Err(Error::Misaligned));
        let id = registry.register(b"x", 2, 1).unwrap();
        registry.unregister(id).unwrap();
        let mut output = [0; 2];
        assert_eq!(registry.initialize(id, &mut output), Err(Error::Stale));
    }

    #[test]
    fn publishes_lazily_and_preserves_thread_local_mutation() {
        let mut registry = Registry::new();
        let id = registry.register(b"abc", 6, 1).unwrap();
        let mut dtv = Dtv::new();
        *dtv.get_addr(&registry, id, 3).unwrap() = 9;
        assert_eq!(
            dtv.block(&registry, id).unwrap(),
            &[b'a', b'b', b'c', 9, 0, 0]
        );
        assert_eq!(*dtv.get_addr(&registry, id, 3).unwrap(), 9);
    }

    #[test]
    fn stale_generation_cannot_reuse_published_storage() {
        let mut registry = Registry::new();
        let old = registry.register(b"old", 3, 1).unwrap();
        let mut dtv = Dtv::new();
        assert_eq!(dtv.block(&registry, old).unwrap(), b"old");
        registry.unregister(old).unwrap();
        let fresh = registry.register(b"new", 3, 1).unwrap();
        assert_eq!(dtv.block(&registry, fresh).unwrap(), b"new");
        assert_eq!(dtv.get_addr(&registry, old, 0), Err(Error::Stale));
    }
}
