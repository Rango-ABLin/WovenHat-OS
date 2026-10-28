/* musl x86_64 syscall architecture override for WovenHat. */
#ifndef WOVENHAT_MUSL_SYSCALL_ARCH_H
#define WOVENHAT_MUSL_SYSCALL_ARCH_H

#include <wovenhat/syscall.h>

#define __SYSCALL_LL_E(x) (x)
#define __SYSCALL_LL_O(x) (x)

/* Linux numbers used by the small initial static-libc bring-up subset. */
#define WH_LINUX_READ       0
#define WH_LINUX_WRITE      1
#define WH_LINUX_CLOSE      3
#define WH_LINUX_EXIT       60
#define WH_LINUX_EXIT_GROUP 231

static __inline long wh_musl_syscall0(long number)
{
	return wh_syscall0(number);
}

static __inline long wh_musl_syscall1(long number, long arg0)
{
	switch (number) {
	case WH_LINUX_CLOSE:
		return wh_syscall1(WH_SYS_CLOSE, arg0);
	case WH_LINUX_EXIT:
	case WH_LINUX_EXIT_GROUP:
		return wh_syscall1(WH_SYS_EXIT, arg0);
	default:
		return -WH_ENOSYS;
	}
}

static __inline long wh_musl_syscall3(long number, long arg0, long arg1, long arg2)
{
	switch (number) {
	case WH_LINUX_READ:
		return wh_syscall3(WH_SYS_READ, arg0, arg1, arg2);
	case WH_LINUX_WRITE:
		return wh_syscall3(WH_SYS_WRITE, arg0, arg1, arg2);
	default:
		return -WH_ENOSYS;
	}
}

/* musl's internal syscall macros call these names. */
#define __syscall0(n) wh_musl_syscall0(n)
#define __syscall1(n,a) wh_musl_syscall1(n,a)
#define __syscall2(n,a,b) (-WH_ENOSYS)
#define __syscall3(n,a,b,c) wh_musl_syscall3(n,a,b,c)
#define __syscall4(n,a,b,c,d) (-WH_ENOSYS)
#define __syscall5(n,a,b,c,d,e) (-WH_ENOSYS)
#define __syscall6(n,a,b,c,d,e,f) (-WH_ENOSYS)

#endif
