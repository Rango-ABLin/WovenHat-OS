/* Static musl-compatible entry for WovenHat's argc/argv/envp/auxv contract. */
#include <stdlib.h>

int main(int, char **, char **);

typedef int (*main_fn)(int, char **, char **);
typedef void (*hook_fn)(void);

int __libc_start_main(main_fn entry, int argc, char **argv,
	hook_fn init, hook_fn fini, hook_fn rtld_fini, void *stack_end)
{
	(void)rtld_fini;
	(void)stack_end;
	char **envp = argv + argc + 1;
	if (init) init();
	int status = entry(argc, argv, envp);
	if (fini) fini();
	return status;
}

_Noreturn void __wovenhat_start_c(long *stack)
{
	int argc = (int)stack[0];
	char **argv = (char **)(stack + 1);
	int status = __libc_start_main((main_fn)main, argc, argv, 0, 0, 0, stack);

	_Exit(status);
}
