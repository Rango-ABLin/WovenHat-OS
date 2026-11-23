#![no_std]

#[unsafe(no_mangle)]
pub extern "C" fn woven_runtime_tls_answer() -> u64 {
    55
}

core::arch::global_asm!(
    ".section .tdata,\"awT\",@progbits",
    ".p2align 3",
    ".global woven_runtime_tls_value",
    "woven_runtime_tls_value:",
    "    .quad 55",
    options(att_syntax),
);
