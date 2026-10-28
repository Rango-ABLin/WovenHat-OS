#include <errno.h>
#include <stddef.h>
#include <unistd.h>
#include <wovenhat/syscall.h>

char *getcwd(char *buffer, size_t size)
{
	long result;

	if (!buffer || !size) {
		errno = EINVAL;
		return 0;
	}
	result = wh_syscall2(WH_SYS_GETCWD, (long)buffer, (long)size);
	if (result < 0) {
		errno = (int)-result;
		return 0;
	}
	return buffer;
}
