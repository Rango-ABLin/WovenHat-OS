#![no_std]

core::arch::global_asm!(
    ".section .tdata,\"awT\",@progbits",
    ".p2align 3",
    ".global woven_tls_value",
    "woven_tls_value:",
    "    .quad 42",
    ".text",
    ".global woven_answer",
    "woven_answer:",
    "    movq woven_tls_value@GOTTPOFF(%rip), %rax",
    "    movq %fs:(%rax), %rax",
    "    ret",
    options(att_syntax),
);
