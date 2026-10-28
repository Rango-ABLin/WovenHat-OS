#include <errno.h>
#include <sys/wait.h>
#include <wovenhat/syscall.h>

/* The current kernel ABI returns a plain child exit code, not a POSIX status. */
pid_t waitpid(pid_t pid, int *status, int options)
{
	long result;
	if (options != 0) {
		errno = ENOSYS;
		return -1;
	}
	do {
		result = wh_syscall1(WH_SYS_WAITPID, pid);
		if (result == -2 && wh_syscall0(WH_SYS_YIELD) < 0) {
			errno = EIO;
			return -1;
		}
	} while (result == -2);
	if (result < 0) {
		errno = (int)-result;
		return -1;
	}
	if (status)
		*status = (int)result;
	return pid;
}
