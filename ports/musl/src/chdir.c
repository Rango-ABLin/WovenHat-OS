/* WovenHat path syscalls use (pointer, explicit byte length). */
#include <errno.h>
#include <unistd.h>
#include <wovenhat/syscall.h>

int chdir(const char *path)
{
	unsigned long length = 0;
	if (!path)
		return (errno = EFAULT), -1;
	while (length < 256 && path[length])
		length++;
	if (!length || length == 256)
		return (errno = ENAMETOOLONG), -1;
	long result = wh_syscall2(WH_SYS_CHDIR, (long)path, (long)length);
	if (result < 0) {
		errno = (int)-result;
		return -1;
	}
	return 0;
}
