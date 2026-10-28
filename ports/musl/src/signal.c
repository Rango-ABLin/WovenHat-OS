#include <errno.h>
#include <signal.h>
#include <wovenhat/syscall.h>

void (*signal(int sig, void (*handler)(int)))(int)
{
	long result = wh_syscall2(WH_SYS_SIGACTION, sig, (long)handler);
	if (result < 0) {
		errno = (int)-result;
		return SIG_ERR;
	}
	return (void (*)(int))result;
}
