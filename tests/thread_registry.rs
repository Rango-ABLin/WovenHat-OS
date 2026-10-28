//! Production thread registry; only IRQ locking and opaque VM payloads are host doubles.
pub mod irq_lock {
    pub struct IrqMutex<T>(std::sync::Mutex<T>);
    impl<T> IrqMutex<T> {
        pub const fn with_rank(value: T, _rank: u8) -> Self {
            Self(std::sync::Mutex::new(value))
        }
        pub fn lock(&self) -> std::sync::MutexGuard<'_, T> {
            self.0.lock().unwrap()
        }
    }
}

pub mod task {
    #[derive(Clone, Copy)]
    pub struct TaskId;

    impl TaskId {
        pub const fn from_u64(value: u64) -> Self {
            let _ = value;
            Self
        }
    }

    pub fn wait_for_event() {}

    pub fn signal_event(_id: TaskId) -> bool {
        true
    }
}

pub mod userspace {
    pub const MAX_DYNAMIC_MODULES: usize = 8;

    #[derive(Clone, Copy)]
    pub struct AnonymousMapping {
        pub address: u64,
    }
}

#[path = "../kernel/src/thread.rs"]
pub mod thread;

#[cfg(test)]
mod dynamic_tls_resource_tests {
    use super::thread::{self, Resources};
    use super::userspace::AnonymousMapping;

    #[test]
    fn dynamic_tls_resource_install_and_clear_are_generation_safe() {
        let id = thread::create(41).expect("thread slot");
        let resources = Resources {
            stack: AnonymousMapping { address: 0x1000 },
            tls: None,
            dynamic_tls: [None; super::userspace::MAX_DYNAMIC_MODULES],
        };
        assert!(thread::attach_resources(id, resources));
        let mapping = AnonymousMapping { address: 0x9000 };
        assert!(thread::install_dynamic_tls(id, 3, mapping));
        assert!(!thread::install_dynamic_tls(id, 3, mapping));
        assert!(!thread::clear_dynamic_tls(id, 3, 0x9001));
        assert!(thread::clear_dynamic_tls(id, 3, mapping.address));
        assert!(!thread::clear_dynamic_tls(id, 3, mapping.address));
        assert!(thread::abort(id));
    }
}
