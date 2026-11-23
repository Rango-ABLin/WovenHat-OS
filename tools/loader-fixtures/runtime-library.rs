#![no_std]

#[unsafe(no_mangle)]
pub static mut WOVEN_RUNTIME_LIFECYCLE_STATE: u64 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn woven_runtime_init() {
    unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(WOVEN_RUNTIME_LIFECYCLE_STATE), 0xa5) };
}

#[unsafe(no_mangle)]
pub extern "C" fn woven_runtime_fini() {
    unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(WOVEN_RUNTIME_LIFECYCLE_STATE), 0x5a) };
}

#[unsafe(no_mangle)]
pub extern "C" fn woven_runtime_answer() -> u64 {
    77
}
