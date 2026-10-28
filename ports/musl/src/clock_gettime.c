#include <errno.h>
#include <time.h>
#include <wovenhat/syscall.h>

int clock_gettime(clockid_t clock_id, struct timespec *result)
{
	if (!result || (clock_id != CLOCK_REALTIME && clock_id != CLOCK_MONOTONIC)) {
		errno = EINVAL;
		return -1;
	}
	long ticks = wh_syscall0(WH_SYS_GETTICKS);
	if (ticks < 0) {
		errno = EIO;
		return -1;
	}
	result->tv_sec = ticks / 100;
	result->tv_nsec = (ticks % 100) * 10000000L;
	return 0;
}
