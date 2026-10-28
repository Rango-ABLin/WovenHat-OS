/*
 * Cancellation-point syscall bridge for the initial WovenHat static-musl
 * subset.  WovenHat has no thread cancellation yet, so this preserves musl's
 * call boundary but does not claim pthread cancellation semantics.
 */
#include <wovenhat/syscall.h>
#include <syscall_arch.h>

long __syscall_cp(long number, long arg0, long arg1, long arg2,
	long arg3, long arg4, long arg5)
{
	(void)arg3;
	(void)arg4;
	(void)arg5;

	switch (number) {
	case WH_LINUX_READ:
	case WH_LINUX_WRITE:
		return __syscall3(number, arg0, arg1, arg2);
	case WH_LINUX_CLOSE:
		return __syscall1(number, arg0);
	default:
		return -WH_ENOSYS;
	}
}
