/* Bounded WovenHat adapter; this is not Linux openat flag emulation. */
#include <fcntl.h>
#include "syscall.h"
#include <wovenhat/syscall.h>

#define WOVENHAT_PATH_MAX 256

int open(const char *path, int flags, ...)
{
	unsigned long length = 0;

	unsigned long access;

	if (!path || flags & ~(O_ACCMODE | O_CREAT))
		return __syscall_ret(-ENOSYS);
	access = (flags & O_ACCMODE) == O_RDONLY ? 1 :
		(flags & O_ACCMODE) == O_WRONLY ? 2 :
		(flags & O_ACCMODE) == O_RDWR ? 3 : 0;
	if (!access)
		return __syscall_ret(-ENOSYS);
	if (flags & O_CREAT)
		access |= 4;
	while (length < WOVENHAT_PATH_MAX && path[length])
		length++;
	if (!length || length == WOVENHAT_PATH_MAX)
		return __syscall_ret(-ENOSYS);
	return __syscall_ret(access == 1
		? wh_syscall2(WH_SYS_OPEN, (long)path, (long)length)
		: wh_syscall3(WH_SYS_OPEN_ACCESS, (long)path, (long)length, (long)access));
}
