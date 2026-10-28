/* Bootstrap errno storage backed by initial-exec TLS. The loader supplies
 * each thread's FS base and the kernel clones the PT_TLS image for threads. */
#include <errno.h>

static __thread int errno_value = 0x1234;

int *__errno_location(void)
{
	return &errno_value;
}

/* musl exposes this compatibility spelling through weak_alias upstream. */
int *___errno_location(void)
{
	return __errno_location();
}
