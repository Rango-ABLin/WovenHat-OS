/* Minimal static C entry for WovenHat's existing argc/argv stack contract. */
#include <stdlib.h>

int main(int, char **);

_Noreturn void __wovenhat_start_c(long *stack)
{
	int status = main((int)stack[0], (char **)(stack + 1));

	_Exit(status);
}
