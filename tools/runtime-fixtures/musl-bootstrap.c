#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
int main(int argc, char **argv)
{
	char byte = 'x';

	if (argc != 1 || argv[0][0] != 's' || argv[0][1] != '0')
		return 92;
	if (write(1, &byte, 1) != 1)
		return 94;
	/* Invalid descriptor must exercise musl's syscall return/errno path. */
	if (write(-1, &byte, 1) != -1 || errno != 5)
		return 91;
	if (close(-1) != -1 || errno != 5)
		return 93;
	if (open("/not-a-wovenhat-open-fixture", O_WRONLY) != -1 || errno != 38)
		return 95;
	return 42;
}
