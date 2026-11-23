#![no_std]

#[repr(C)]
pub struct TlsIndex {
    module: u64,
    offset: u64,
}

unsafe extern "C" {
    fn woven_runtime_dependency_tls_value() -> u64;
}

#[unsafe(no_mangle)]
pub extern "C" fn __tls_get_addr(index: *const TlsIndex) -> *mut u8 {
    let index = unsafe { core::ptr::read(index) };
    let module = index.module;
    let offset = index.offset;
    let mut address = 141_u64;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") address,
            in("rdi") module,
            in("rsi") offset,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    address as *mut u8
}

#[unsafe(no_mangle)]
pub extern "C" fn woven_runtime_compiler_tls_probe() -> u64 {
    unsafe { woven_runtime_dependency_tls_value() }
}
