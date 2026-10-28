/* Bounded WovenHat adapter; this is not Linux openat flag emulation. */
#include <fcntl.h>
#include "syscall.h"
#include <wovenhat/syscall.h>

#define WOVENHAT_PATH_MAX 256

int open(const char *path, int flags, ...)
{
	unsigned long length = 0;

	/* Kernel open is read-only path+length today; do not discard flags/mode. */
	if (!path || flags != O_RDONLY)
		return __syscall_ret(-ENOSYS);
	while (length < WOVENHAT_PATH_MAX && path[length])
		length++;
	if (!length || length == WOVENHAT_PATH_MAX)
		return __syscall_ret(-ENOSYS);
	return __syscall_ret(wh_syscall2(WH_SYS_OPEN, (long)path, (long)length));
}
