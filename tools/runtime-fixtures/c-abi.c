/*
 * WovenHat S02 freestanding C ABI fixture.
 *
 * This is intentionally not a hosted-C program: no libc headers, startup
 * objects, allocator, TLS, errno or compiler runtime are required. The
 * kernel's Ring-3 entry ABI transfers control to _start with a 16-byte aligned
 * stack. Exit status is the native WovenHat syscall ABI used by the existing
 * independent Rust fixture.
 */

#include "wovenhat/syscall.h"

typedef unsigned long long wh_u64;

static wh_u64 wh_exit_status(void) {
    return 42;
}

__attribute__((noreturn)) void _start(void) {
    wh_u64 status = wh_exit_status();
    (void)wh_syscall1(WH_SYS_EXIT, (long)status);
    __asm__ volatile ("ud2" ::: "memory");
    __builtin_unreachable();
}
