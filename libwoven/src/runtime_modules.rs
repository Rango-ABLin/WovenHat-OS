//! Bounded runtime module lifecycle joining namespace and dynamic TLS state.

use crate::dynamic_tls::{Dtv, Error as TlsError, ModuleId as TlsModuleId, Registry as TlsRegistry};
use crate::module_namespace::{Error as NamespaceError, ModuleId, Namespace};

pub const MAX_MODULES: usize = crate::dynamic_tls::MAX_MODULES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
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
}
