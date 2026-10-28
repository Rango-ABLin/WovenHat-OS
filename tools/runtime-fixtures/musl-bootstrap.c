#include <errno.h>
#include <unistd.h>
#include <wovenhat/syscall.h>

_Noreturn void _start(void)
{
	char byte = 'x';

	/* Invalid descriptor must exercise musl's syscall return/errno path. */
	if (write(-1, &byte, 1) != -1 || errno != 5)
		(void)wh_syscall1(WH_SYS_EXIT, 91);
	(void)wh_syscall1(WH_SYS_EXIT, 42);
	__asm__ __volatile__("ud2" ::: "memory");
	__builtin_unreachable();
}
