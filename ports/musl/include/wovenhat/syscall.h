/*
 * WovenHat's musl-port syscall transport.
 *
 * This header is deliberately not a Linux syscall-number compatibility layer.
 * WovenHat's DPL3 gate accepts at most three arguments in RDI, RSI and RDX,
 * and returns UINT64_MAX on failure.  Each musl-facing wrapper therefore must
 * be added only after its Linux argument shape has a checked WovenHat mapping.
 */
#ifndef WOVENHAT_MUSL_SYSCALL_H
#define WOVENHAT_MUSL_SYSCALL_H

/* Current WovenHat syscall numbers; keep synchronized with kernel::syscall. */
#define WH_SYS_READ  0
#define WH_SYS_WRITE 1
#define WH_SYS_OPEN  2
#define WH_SYS_GETPID 4
#define WH_SYS_EXIT  3
#define WH_SYS_CLOSE 6
#define WH_SYS_YIELD 7
#define WH_SYS_WAITPID 5
#define WH_SYS_FORK 16
#define WH_SYS_SIGACTION 34
#define WH_SYS_KILL 26
#define WH_SYS_GETPPID 25
#define WH_SYS_GETUID 13
#define WH_SYS_GETGID 14
#define WH_SYS_CHDIR 20
#define WH_SYS_GETCWD 21
#define WH_SYS_GETTICKS 31
#define WH_SYS_LSEEK 27
#define WH_SYS_FILE_WRITE 10
#define WH_SYS_OPEN_ACCESS 254
#define WH_SYS_DUP 22
#define WH_SYS_DUP2 24
#define WH_SYS_PIPE 23
#define WH_SYS_MMAP 8
#define WH_SYS_MUNMAP 9
#define WH_SYS_THREAD_CREATE 130
#define WH_SYS_THREAD_EXIT 131
#define WH_SYS_THREAD_JOIN 132
#define WH_SYS_THREAD_SELF 133
#define WH_SYS_THREAD_CANCEL 136
#define WH_SYS_THREAD_SLEEP 138
#define WH_SYS_THREAD_CONDITION_WAIT 139
#define WH_SYS_THREAD_CONDITION_SIGNAL 140
#define WH_SYS_DYNAMIC_TLS_ADDR 141

/* Linux-compatible negative result used when a mapping is not implemented. */
#define WH_ENOSYS 38
#define WH_EPIPE 32
#define WH_EINTR 4

static __inline long wh_syscall0(long number)
{
	unsigned long result;
	__asm__ __volatile__("int $0x80"
		: "=a"(result) : "a"(number) : "memory");
	return result == ~0UL ? -5 : result == ~0UL-2 ? -WH_EPIPE : result == ~0UL-3 ? -WH_EINTR : (long)result;
}

static __inline long wh_syscall1(long number, long arg0)
{
	unsigned long result;
	__asm__ __volatile__("int $0x80"
		: "=a"(result) : "a"(number), "D"(arg0) : "memory");
	return result == ~0UL ? -5 : result == ~0UL-2 ? -WH_EPIPE : result == ~0UL-3 ? -WH_EINTR : (long)result;
}

static __inline long wh_syscall2(long number, long arg0, long arg1)
{
	unsigned long result;
	__asm__ __volatile__("int $0x80"
		: "=a"(result) : "a"(number), "D"(arg0), "S"(arg1) : "memory");
	return result == ~0UL ? -5 : result == ~0UL-2 ? -WH_EPIPE : result == ~0UL-3 ? -WH_EINTR : (long)result;
}

static __inline long wh_syscall3(long number, long arg0, long arg1, long arg2)
{
	unsigned long result;
	__asm__ __volatile__("int $0x80"
		: "=a"(result)
		: "a"(number), "D"(arg0), "S"(arg1), "d"(arg2)
		: "memory");
	return result == ~0UL ? -5 : result == ~0UL-2 ? -WH_EPIPE : result == ~0UL-3 ? -WH_EINTR : (long)result;
}

/* Never discard ABI arguments. Higher-arity mappings require a dedicated ABI. */
static __inline long wh_syscall4_unmapped(void) { return -WH_ENOSYS; }
static __inline long wh_syscall5_unmapped(void) { return -WH_ENOSYS; }
static __inline long wh_syscall6_unmapped(void) { return -WH_ENOSYS; }

#endif
