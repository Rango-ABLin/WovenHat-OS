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

pub mod userspace {
    #[derive(Clone, Copy)]
    pub struct AnonymousMapping;
}

#[path = "../kernel/src/thread.rs"]
pub mod thread;
