/* musl x86_64 syscall architecture override for WovenHat. */
#ifndef WOVENHAT_MUSL_SYSCALL_ARCH_H
#define WOVENHAT_MUSL_SYSCALL_ARCH_H

#include <wovenhat/syscall.h>

#define __SYSCALL_LL_E(x) (x)
#define __SYSCALL_LL_O(x) (x)

/* Linux numbers used by the small initial static-libc bring-up subset. */
#define WH_LINUX_READ       0
#define WH_LINUX_WRITE      1
#define WH_LINUX_GETPID     39
#define WH_LINUX_CLOSE      3
#define WH_LINUX_GETPPID    110
#define WH_LINUX_LSEEK      8
#define WH_LINUX_DUP        32
#define WH_LINUX_DUP2       33
#define WH_LINUX_EXIT       60
#define WH_LINUX_EXIT_GROUP 231

static __inline long __syscall0(long number)
{
	return wh_syscall0(number);
}

static __inline long __syscall1(long number, long arg0)
{
	switch (number) {
	case WH_LINUX_GETPID:
		return wh_syscall0(WH_SYS_GETPID);
	case WH_LINUX_GETPPID:
		return wh_syscall0(WH_SYS_GETPPID);
	case WH_LINUX_CLOSE:
		return wh_syscall1(WH_SYS_CLOSE, arg0);
	case WH_LINUX_DUP:
		return wh_syscall1(WH_SYS_DUP, arg0);
	case WH_LINUX_EXIT:
	case WH_LINUX_EXIT_GROUP:
		return wh_syscall1(WH_SYS_EXIT, arg0);
	default:
		return -WH_ENOSYS;
	}
}

static __inline long __syscall2(long number, long arg0, long arg1)
{
	return number == WH_LINUX_DUP2 ? wh_syscall2(WH_SYS_DUP2, arg0, arg1) : -WH_ENOSYS;
}

static __inline long __syscall3(long number, long arg0, long arg1, long arg2)
{
	switch (number) {
	case WH_LINUX_READ:
		return wh_syscall3(WH_SYS_READ, arg0, arg1, arg2);
	case WH_LINUX_WRITE:
		return wh_syscall3(WH_SYS_WRITE, arg0, arg1, arg2);
	case WH_LINUX_LSEEK:
		return arg2 == 0 ? wh_syscall2(WH_SYS_LSEEK, arg0, arg1) : -WH_ENOSYS;
	default:
		return -WH_ENOSYS;
	}
}

static __inline long __syscall4(long n, long a, long b, long c, long d)
{
	(void)n; (void)a; (void)b; (void)c; (void)d;
	return -WH_ENOSYS;
}

static __inline long __syscall5(long n, long a, long b, long c, long d, long e)
{
	(void)n; (void)a; (void)b; (void)c; (void)d; (void)e;
	return -WH_ENOSYS;
}

static __inline long __syscall6(long n, long a, long b, long c, long d, long e, long f)
{
	(void)n; (void)a; (void)b; (void)c; (void)d; (void)e; (void)f;
	return -WH_ENOSYS;
}

#endif
