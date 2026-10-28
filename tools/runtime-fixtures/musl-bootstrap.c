#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
int main(int argc, char **argv)
{
	char byte = 'x';
	char motd[24];
	int fd;

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
	fd = open("/etc/motd", O_RDONLY);
	if (fd < 3)
		return 96;
	if (read(fd, motd, sizeof motd) != (int)sizeof motd
		|| motd[0] != 'W' || motd[8] != 't' || motd[21] != 'S'
		|| motd[23] != '\n')
		return 98;
	if (read(fd, &byte, 1) != 0)
		return 99;
	if (close(fd) != 0)
		return 97;
	return 42;
}
