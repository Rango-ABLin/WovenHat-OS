//! Bounded dynamic-TLS template and per-thread storage foundation.

pub const MAX_MODULES: usize = 8;
pub const MAX_TLS_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleId { slot: u8, generation: u32 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Capacity, Invalid, Stale, Misaligned, TooLarge, BufferTooSmall }

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
    const EMPTY: Self = Self { generation: 1, live: false, memory_size: 0, alignment: 1, init: [0; MAX_TLS_BYTES], init_size: 0 };
}

pub struct Registry { modules: [Module; MAX_MODULES] }
impl Registry {
    pub const fn new() -> Self { Self { modules: [Module::EMPTY; MAX_MODULES] } }
    pub fn register(&mut self, initializer: &[u8], memory_size: usize, alignment: usize) -> Result<ModuleId, Error> {
        if initializer.len() > memory_size || memory_size > MAX_TLS_BYTES { return Err(Error::TooLarge); }
        if !alignment.is_power_of_two() || alignment > MAX_TLS_BYTES { return Err(Error::Misaligned); }
        let (slot, module) = self.modules.iter_mut().enumerate().find(|(_, m)| !m.live).ok_or(Error::Capacity)?;
        module.live = true; module.memory_size = memory_size; module.alignment = alignment;
        module.init_size = initializer.len(); module.init[..initializer.len()].copy_from_slice(initializer);
        Ok(ModuleId { slot: slot as u8, generation: module.generation })
    }
    pub fn unregister(&mut self, id: ModuleId) -> Result<(), Error> {
        let module = self.modules.get_mut(id.slot as usize).ok_or(Error::Invalid)?;
        if !module.live || module.generation != id.generation { return Err(Error::Stale); }
        module.live = false; module.generation = module.generation.saturating_add(1).max(1);
        module.memory_size = 0; module.init_size = 0; Ok(())
    }
    pub fn initialize(&self, id: ModuleId, output: &mut [u8]) -> Result<usize, Error> {
        let module = self.modules.get(id.slot as usize).ok_or(Error::Invalid)?;
        if !module.live || module.generation != id.generation { return Err(Error::Stale); }
        if output.len() < module.memory_size { return Err(Error::BufferTooSmall); }
        output[..module.memory_size].fill(0);
        output[..module.init_size].copy_from_slice(&module.init[..module.init_size]);
        Ok(module.memory_size)
    }
    pub fn alignment(&self, id: ModuleId) -> Result<usize, Error> {
        let module = self.modules.get(id.slot as usize).ok_or(Error::Invalid)?;
        (module.live && module.generation == id.generation).then_some(module.alignment).ok_or(Error::Stale)
    }
}
impl Default for Registry { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::{Error, Registry};
    #[test]
    fn initializes_and_zero_fills_each_thread_block() {
        let mut registry = Registry::new(); let id = registry.register(b"tls", 8, 8).unwrap();
        let mut output = [0xaa; 8]; assert_eq!(registry.initialize(id, &mut output), Ok(8));
        assert_eq!(&output, b"tls\0\0\0\0\0"); assert_eq!(registry.alignment(id), Ok(8));
    }
    #[test]
    fn rejects_invalid_templates_and_stale_generations() {
        let mut registry = Registry::new();
        assert_eq!(registry.register(b"long", 2, 4), Err(Error::TooLarge));
        assert_eq!(registry.register(b"x", 2, 3), Err(Error::Misaligned));
        let id = registry.register(b"x", 2, 1).unwrap(); registry.unregister(id).unwrap();
        let mut output = [0; 2]; assert_eq!(registry.initialize(id, &mut output), Err(Error::Stale));
    }
}
