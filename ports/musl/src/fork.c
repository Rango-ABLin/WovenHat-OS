#include <errno.h>
#include <unistd.h>
#include <wovenhat/syscall.h>

/* The fork gate snapshots the saved int80 frame and returns zero in the child.
 * Threaded atfork machinery is outside this single-thread bootstrap profile. */
pid_t fork(void)
{
	long result = wh_syscall0(WH_SYS_FORK);
	if (result < 0) {
		errno = (int)-result;
		return -1;
	}
	return (pid_t)result;
}
