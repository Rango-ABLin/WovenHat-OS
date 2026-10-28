//! Fixed-capacity unload-safe module namespace foundation.
//!
//! This layer owns identity and references only. Mapping, relocation, TLS,
//! constructors and destructors remain loader authorities and must publish
//! through this boundary transactionally.

pub const MAX_MODULES: usize = 8;
pub const MAX_SYMBOLS: usize = 32;
pub const MAX_NAME: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleId {
    slot: u8,
    generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    Invalid,
    Duplicate,
    NameTooLong,
    NotFound,
    StillReferenced,
}

#[derive(Clone, Copy)]
struct Module {
    generation: u32,
    live: bool,
    references: u16,
}

impl Module {
    const EMPTY: Self = Self { generation: 1, live: false, references: 0 };
}

#[derive(Clone, Copy)]
struct Symbol {
    module: ModuleId,
    length: u8,
    name: [u8; MAX_NAME],
    address: u64,
}

impl Symbol {
    const EMPTY: Self = Self {
        module: ModuleId { slot: 0, generation: 0 },
        length: 0,
        name: [0; MAX_NAME],
        address: 0,
    };
}

pub struct Namespace {
    modules: [Module; MAX_MODULES],
    symbols: [Option<Symbol>; MAX_SYMBOLS],
}

impl Namespace {
    pub const fn new() -> Self {
        Self { modules: [Module::EMPTY; MAX_MODULES], symbols: [None; MAX_SYMBOLS] }
    }

    fn valid(&self, id: ModuleId) -> bool {
        self.modules
            .get(id.slot as usize)
            .is_some_and(|module| module.live && module.generation == id.generation)
    }

    pub fn load(&mut self) -> Result<ModuleId, Error> {
        let (slot, module) = self
            .modules
            .iter_mut()
            .enumerate()
            .find(|(_, module)| !module.live)
            .ok_or(Error::Capacity)?;
        module.live = true;
        module.references = 1;
        Ok(ModuleId { slot: slot as u8, generation: module.generation })
    }

    pub fn retain(&mut self, id: ModuleId) -> Result<(), Error> {
        let module = self.modules.get_mut(id.slot as usize).ok_or(Error::Invalid)?;
        if !module.live || module.generation != id.generation {
            return Err(Error::Invalid);
        }
        module.references = module.references.checked_add(1).ok_or(Error::Capacity)?;
        Ok(())
    }

    /// Drop one reference. Returns true when the module became unloadable.
    pub fn release(&mut self, id: ModuleId) -> Result<bool, Error> {
        let module = self.modules.get_mut(id.slot as usize).ok_or(Error::Invalid)?;
        if !module.live || module.generation != id.generation || module.references == 0 {
            return Err(Error::Invalid);
        }
        module.references -= 1;
        Ok(module.references == 0)
    }

    /// Retire a zero-reference module and invalidate all its symbols.
    pub fn unload(&mut self, id: ModuleId) -> Result<(), Error> {
        let module = self.modules.get_mut(id.slot as usize).ok_or(Error::Invalid)?;
        if !module.live || module.generation != id.generation {
            return Err(Error::Invalid);
        }
        if module.references != 0 {
            return Err(Error::StillReferenced);
        }
        module.live = false;
        module.generation = module.generation.saturating_add(1).max(1);
        for symbol in &mut self.symbols {
            if symbol.is_some_and(|symbol| symbol.module == id) {
                *symbol = None;
            }
        }
        Ok(())
    }

    pub fn publish_symbol(&mut self, id: ModuleId, name: &[u8], address: u64) -> Result<(), Error> {
        if !self.valid(id) {
            return Err(Error::Invalid);
        }
        if name.is_empty() || name.len() > MAX_NAME {
            return Err(Error::NameTooLong);
        }
        if self.symbols.iter().flatten().any(|symbol| symbol.name[..symbol.length as usize] == *name) {
            return Err(Error::Duplicate);
        }
        let slot = self.symbols.iter().position(Option::is_none).ok_or(Error::Capacity)?;
        let mut symbol = Symbol::EMPTY;
        symbol.module = id;
        symbol.length = name.len() as u8;
        symbol.name[..name.len()].copy_from_slice(name);
        symbol.address = address;
        self.symbols[slot] = Some(symbol);
        Ok(())
    }

    pub fn lookup(&self, name: &[u8]) -> Result<(ModuleId, u64), Error> {
        self.symbols
            .iter()
            .flatten()
            .find(|symbol| symbol.name[..symbol.length as usize] == *name && self.valid(symbol.module))
            .map(|symbol| (symbol.module, symbol.address))
            .ok_or(Error::NotFound)
    }
}

impl Default for Namespace {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::{Error, Namespace, MAX_MODULES};

    #[test]
    fn stale_handles_cannot_resolve_after_unload_and_reuse() {
        let mut namespace = Namespace::new();
        let first = namespace.load().unwrap();
        namespace.publish_symbol(first, b"entry", 0x1000).unwrap();
        assert_eq!(namespace.lookup(b"entry").unwrap().1, 0x1000);
        assert_eq!(namespace.release(first), Ok(true));
        namespace.unload(first).unwrap();
        assert_eq!(namespace.lookup(b"entry"), Err(Error::NotFound));
        let second = namespace.load().unwrap();
        assert_ne!(first, second);
        assert_eq!(namespace.retain(first), Err(Error::Invalid));
    }

    #[test]
    fn references_gate_unload_and_capacity_is_bounded() {
        let mut namespace = Namespace::new();
        let mut ids = [None; MAX_MODULES];
        for id in &mut ids { *id = Some(namespace.load().unwrap()); }
        assert_eq!(namespace.load(), Err(Error::Capacity));
        let id = ids[0].unwrap();
        namespace.retain(id).unwrap();
        assert_eq!(namespace.release(id), Ok(false));
        assert_eq!(namespace.unload(id), Err(Error::StillReferenced));
        assert_eq!(namespace.release(id), Ok(true));
        namespace.unload(id).unwrap();
    }
}
