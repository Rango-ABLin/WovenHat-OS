#include <errno.h>
#include <signal.h>
#include <wovenhat/syscall.h>

int kill(pid_t pid, int sig)
{
	long result = wh_syscall2(WH_SYS_KILL, pid, sig);
	if (result < 0) {
		errno = (int)-result;
		return -1;
	}
	return 0;
}
