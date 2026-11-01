// Low-level x86_64 Context Switcher using global inline assembly for WovenHat-OS
use core::arch::global_asm;

global_asm!(
    r#".global switch_context
    .type switch_context, @function
    switch_context:
        push %rbx
        push %rbp
        push %r12
        push %r13
        push %r14
        push %r15

        mov %rsp, (%rdi)
        mov (%rsi), %rsp

        pop %r15
        pop %r14
        pop %r13
        pop %r12
        pop %rbp
        pop %rbx

        ret
    "#
);

extern "C" {
    pub fn switch_context(old_stack_ptr: *mut u64, new_stack_ptr: *const u64);
}
