#include <errno.h>
#include <signal.h>
#include <wovenhat/syscall.h>

void (*signal(int sig, void (*handler)(int)))(int)
{
	/* signal() has SA_RESTART semantics; bit 62 is the private ABI flag. */
	long result = wh_syscall2(WH_SYS_SIGACTION, sig,
		(long)((unsigned long)handler | (1UL << 62)));
	if (result < 0) {
		errno = (int)-result;
		return SIG_ERR;
	}
	return (void (*)(int))result;
}

/* Bootstrap-only hook for proving the non-restart EINTR path. */
void (*__wovenhat_signal_no_restart(int sig, void (*handler)(int)))(int)
{
	long result = wh_syscall2(WH_SYS_SIGACTION, sig, (long)handler);
	if (result < 0) {
		errno = (int)-result;
		return SIG_ERR;
	}
	return (void (*)(int))result;
}
