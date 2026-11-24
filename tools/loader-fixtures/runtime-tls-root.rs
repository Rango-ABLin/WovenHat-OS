#![no_std]

#[repr(C)]
pub struct TlsIndex {
    module: u64,
    offset: u64,
}

unsafe extern "C" {
    fn woven_runtime_dependency_tls_value() -> u64;
    fn woven_runtime_dependency_tlsdesc_value() -> u64;
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
    let global_dynamic = unsafe { woven_runtime_dependency_tls_value() };
    let tlsdesc = unsafe { woven_runtime_dependency_tlsdesc_value() };
    if global_dynamic == tlsdesc {
        global_dynamic
    } else {
        u64::MAX
    }
}

core::arch::global_asm!(
    ".text",
    ".global __woven_tlsdesc_resolver",
    ".type __woven_tlsdesc_resolver,@function",
    "__woven_tlsdesc_resolver:",
    "    movq 8(%rax), %rdx",
    "    movl %edx, %esi",
    "    andl $0xfff, %esi",
    "    movl %edx, %edi",
    "    shrl $12, %edi",
    "    movabsq $0xffffffff00000000, %rcx",
    "    andq %rdx, %rcx",
    "    orq %rcx, %rdi",
    "    movl $141, %eax",
    "    int $0x80",
    "    movq %fs:0, %rcx",
    "    subq %rcx, %rax",
    "    ret",
    options(att_syntax),
);
