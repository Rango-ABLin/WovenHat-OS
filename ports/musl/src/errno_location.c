/*
 * Bootstrap errno storage before WovenHat has musl TLS and pthread state.
 *
 * This is intentionally one process-wide cell.  It is sufficient only for
 * the single-threaded static-libc bring-up fixture; it must be replaced by
 * the upstream pthread/TLS implementation before threads are enabled.
 */
#include <errno.h>

int *__errno_location(void)
{
	static int errno_value;

	return &errno_value;
}

/* musl exposes this compatibility spelling through weak_alias upstream. */
int *___errno_location(void)
{
	return __errno_location();
}
