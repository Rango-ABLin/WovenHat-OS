/* WovenHat returns pipe fds packed as low/high u32, avoiding user-pointer ABI. */
#include <unistd.h>
#include "syscall.h"
#include <wovenhat/syscall.h>

int pipe(int fd[2])
{
	long packed;

	if (!fd)
		return __syscall_ret(-14);
	packed = wh_syscall0(WH_SYS_PIPE);
	if (packed < 0)
		return __syscall_ret(packed);
	fd[0] = (int)(unsigned long)packed;
	fd[1] = (int)((unsigned long)packed >> 32);
	return 0;
}
