#![no_std]
#![feature(thread_local)]

#[thread_local]
#[unsafe(no_mangle)]
pub static mut WOVEN_RUNTIME_DEPENDENCY_TLS: u64 = 91;

#[unsafe(no_mangle)]
pub extern "C" fn woven_runtime_dependency_tls_value() -> u64 {
    // Keep a real compiler-generated TLS access in the shared object.
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(WOVEN_RUNTIME_DEPENDENCY_TLS)) }
}
