/* Minimal Linux syscall names accepted by the WovenHat musl transport. */
#ifndef WOVENHAT_MUSL_BITS_SYSCALL_H
#define WOVENHAT_MUSL_BITS_SYSCALL_H

#define SYS_read       0
#define SYS_write      1
#define SYS_close      3
#define SYS_exit       60
#define SYS_exit_group 231
#define SYS_lseek      8

#endif
