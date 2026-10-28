/*
 * Bootstrap errno storage backed by initial-exec TLS. The loader supplies
 * each thread's FS base and the kernel clones the PT_TLS image for threads.
 */
#include <errno.h>

int *__errno_location(void)
{
	static _Thread_local int errno_value;

	return &errno_value;
}

/* musl exposes this compatibility spelling through weak_alias upstream. */
int *___errno_location(void)
{
	return __errno_location();
}
