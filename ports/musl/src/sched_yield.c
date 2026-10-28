#include <sched.h>
#include <errno.h>
#include <wovenhat/syscall.h>

int sched_yield(void)
{
	long result = wh_syscall0(WH_SYS_YIELD);
	if (result < 0) {
		errno = (int)-result;
		return -1;
	}
	return 0;
}
