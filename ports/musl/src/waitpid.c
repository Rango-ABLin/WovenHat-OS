#include <errno.h>
#include <sys/wait.h>
#include <wovenhat/syscall.h>

/* Translate the kernel's plain exit code into the POSIX wait-status layout. */
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
	if (status) {
		int exit_code = (int)result;
		*status = exit_code >= 128 ? exit_code - 128 : exit_code << 8;
	}
	return pid;
}
