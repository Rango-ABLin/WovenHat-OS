//! Bounded runtime module lifecycle joining namespace and dynamic TLS state.

use crate::dynamic_tls::{Dtv, Error as TlsError, ModuleId as TlsModuleId, Registry as TlsRegistry};
use crate::module_namespace::{Error as NamespaceError, ModuleId, Namespace};

pub const MAX_MODULES: usize = crate::dynamic_tls::MAX_MODULES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    InvalidImage,
    MissingTls,
    Namespace(NamespaceError),
    Tls(TlsError),
    Stale,
    StillReferenced,
}

#[derive(Clone, Copy)]
struct Pair {
    namespace: ModuleId,
    tls: TlsModuleId,
}

pub struct RuntimeModules {
    namespace: Namespace,
    tls: TlsRegistry,
    pairs: [Option<Pair>; MAX_MODULES],
}

impl RuntimeModules {
    pub const fn new() -> Self {
        Self { namespace: Namespace::new(), tls: TlsRegistry::new(), pairs: [None; MAX_MODULES] }
    }

    /// Load a module's namespace identity and TLS template transactionally.
    pub fn load(&mut self, initializer: &[u8], memory_size: usize, alignment: usize) -> Result<ModuleId, Error> {
        let namespace = self.namespace.load().map_err(|error| match error {
            NamespaceError::Capacity => Error::Capacity,
            other => Error::Namespace(other),
        })?;
        let tls = match self.tls.register(initializer, memory_size, alignment) {
            Ok(id) => id,
            Err(error) => {
                let _ = self.namespace.unload(namespace);
                return Err(Error::Tls(error));
            }
        };
        let Some(slot) = self.pairs.iter().position(Option::is_none) else {
            let _ = self.tls.unregister(tls);
            let _ = self.namespace.unload(namespace);
            return Err(Error::Capacity);
        };
        self.pairs[slot] = Some(Pair { namespace, tls });
        Ok(namespace)
    }

    /// Parse and register the bounded PT_TLS payload from an ELF64 image.
    /// Code mapping and relocation remain kernel loader responsibilities.
    pub fn load_elf_tls(&mut self, image: &[u8]) -> Result<ModuleId, Error> {
        const PT_TLS: u32 = 7;
        if image.len() < 64 || &image[..4] != b"\x7fELF" || image[4] != 2 || image[5] != 1 {
            return Err(Error::InvalidImage);
        }
        let phoff = read_u64(image, 32).ok_or(Error::InvalidImage)? as usize;
        let phentsize = read_u16(image, 54).ok_or(Error::InvalidImage)? as usize;
        let phnum = read_u16(image, 56).ok_or(Error::InvalidImage)? as usize;
        if phentsize < 56 || phnum > 64 {
            return Err(Error::InvalidImage);
        }
        for index in 0..phnum {
            let header = phoff.checked_add(index.checked_mul(phentsize).ok_or(Error::InvalidImage)?).ok_or(Error::InvalidImage)?;
            if header.checked_add(56).ok_or(Error::InvalidImage)? > image.len() {
                return Err(Error::InvalidImage);
            }
            if read_u32(image, header) != Some(PT_TLS) {
                continue;
            }
            let offset = read_u64(image, header + 8).ok_or(Error::InvalidImage)? as usize;
            let filesz = read_u64(image, header + 32).ok_or(Error::InvalidImage)? as usize;
            let memory_size = read_u64(image, header + 40).ok_or(Error::InvalidImage)? as usize;
            let alignment = read_u64(image, header + 48).ok_or(Error::InvalidImage)? as usize;
            let end = offset.checked_add(filesz).ok_or(Error::InvalidImage)?;
            if end > image.len() || filesz > memory_size || memory_size == 0 {
                return Err(Error::InvalidImage);
            }
            return self.load(&image[offset..end], memory_size, alignment.max(1));
        }
        Err(Error::MissingTls)
    }

    pub fn retain(&mut self, id: ModuleId) -> Result<(), Error> {
        self.namespace.retain(id).map_err(Error::Namespace)
    }

    /// Publish one verified symbol into the module namespace.
    pub fn publish_symbol(&mut self, id: ModuleId, name: &[u8], address: u64) -> Result<(), Error> {
        self.namespace
            .publish_symbol(id, name, address)
            .map_err(Error::Namespace)
    }

    /// Resolve a symbol while retaining the namespace's generation check.
    pub fn lookup_symbol(&self, name: &[u8]) -> Result<(ModuleId, u64), Error> {
        self.namespace.lookup(name).map_err(|error| match error {
            NamespaceError::NotFound => Error::Stale,
            other => Error::Namespace(other),
        })
    }

    pub fn release(&mut self, id: ModuleId) -> Result<bool, Error> {
        self.namespace.release(id).map_err(Error::Namespace)
    }

    /// Unload only after all namespace references have been released.
    pub fn unload(&mut self, id: ModuleId) -> Result<(), Error> {
        let slot = self.pairs.iter().position(|pair| pair.is_some_and(|pair| pair.namespace == id)).ok_or(Error::Stale)?;
        self.namespace.unload(id).map_err(|error| match error {
            NamespaceError::StillReferenced => Error::StillReferenced,
            other => Error::Namespace(other),
        })?;
        let pair = self.pairs[slot].take().ok_or(Error::Stale)?;
        self.tls.unregister(pair.tls).map_err(Error::Tls)
    }

    /// Resolve a module-relative TLS address in one thread's DTV.
    pub fn tls_addr<'a>(&self, dtv: &'a mut Dtv, id: ModuleId, offset: usize) -> Result<&'a mut u8, Error> {
        let pair = self.pairs.iter().flatten().find(|pair| pair.namespace == id).ok_or(Error::Stale)?;
        dtv.get_addr(&self.tls, pair.tls, offset).map_err(Error::Tls)
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(offset..offset + 8)?.try_into().ok()?))
}

impl Default for RuntimeModules { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::{Error, RuntimeModules};
    use crate::dynamic_tls::Dtv;

    #[test]
    fn lifecycle_is_transactional_and_tls_is_per_thread() {
        let mut modules = RuntimeModules::new();
        let id = modules.load(b"abc", 4, 4).unwrap();
        modules.publish_symbol(id, b"tls_value", 0x1234).unwrap();
        assert_eq!(modules.lookup_symbol(b"tls_value").unwrap(), (id, 0x1234));
        let mut first = Dtv::new();
        let mut second = Dtv::new();
        *modules.tls_addr(&mut first, id, 3).unwrap() = 7;
        assert_eq!(*modules.tls_addr(&mut first, id, 3).unwrap(), 7);
        assert_eq!(*modules.tls_addr(&mut second, id, 3).unwrap(), 0);
        modules.retain(id).unwrap();
        assert_eq!(modules.unload(id), Err(Error::StillReferenced));
        assert!(!modules.release(id).unwrap());
        assert!(modules.release(id).unwrap());
        modules.unload(id).unwrap();
        assert_eq!(modules.tls_addr(&mut first, id, 0), Err(Error::Stale));
        assert_eq!(modules.lookup_symbol(b"tls_value"), Err(Error::Stale));
    }

    #[test]
    fn parses_bounded_elf_tls_template() {
        let mut image = vec![0u8; 0x120];
        image[..4].copy_from_slice(b"\x7fELF");
        image[4] = 2;
        image[5] = 1;
        image[32..40].copy_from_slice(&64u64.to_le_bytes());
        image[54..56].copy_from_slice(&56u16.to_le_bytes());
        image[56..58].copy_from_slice(&1u16.to_le_bytes());
        image[64..68].copy_from_slice(&7u32.to_le_bytes());
        image[72..80].copy_from_slice(&0x100u64.to_le_bytes());
        image[96..104].copy_from_slice(&3u64.to_le_bytes());
        image[104..112].copy_from_slice(&8u64.to_le_bytes());
        image[112..120].copy_from_slice(&8u64.to_le_bytes());
        image[0x100..0x103].copy_from_slice(b"tls");
        let mut modules = RuntimeModules::new();
        let id = modules.load_elf_tls(&image).unwrap();
        let mut dtv = Dtv::new();
        assert_eq!(*modules.tls_addr(&mut dtv, id, 0).unwrap(), b't');
    }
}
