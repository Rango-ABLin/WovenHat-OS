/*
 * Bootstrap errno storage backed by the loader's initial-exec PT_TLS area.
 * This supplies per-thread errno without claiming pthread creation or the
 * rest of musl's TLS runtime.
 */
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
