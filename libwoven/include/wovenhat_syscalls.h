// C Bindings for Type-Safe Syscalls
#ifndef WOVENHAT_SYSCALLS_H
#define WOVENHAT_SYSCALLS_H

#include <stdint.h>

#define SYSCALL_ERROR ((uint64_t)-1)

uint64_t woven_completion_port_create(void);
int32_t woven_completion_port_close(uint64_t port);
int32_t woven_completion_port_associate(uint64_t op, uint64_t port, uint64_t cookie);
uint64_t woven_getpid(void);
uint64_t woven_getticks(void);
void woven_exit(int32_t status) __attribute__((noreturn));

#endif
