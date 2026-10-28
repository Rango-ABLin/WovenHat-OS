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
	fd = open("/etc/motd", O_RDONLY);
	if (fd < 3)
		return 96;
	if (read(fd, motd, 7) != 7 || read(fd, motd + 7, 17) != 17
		|| motd[0] != 'W' || motd[8] != 't' || motd[21] != 'S'
		|| motd[23] != '\n')
		return 98;
	if (read(fd, &byte, 1) != 0)
		return 99;
	if (lseek(fd, 7, SEEK_SET) != 7 || read(fd, &byte, 1) != 1 || byte != ' ')
		return 100;
	if (close(fd) != 0)
		return 97;
	fd = open("/tmp/musl-bootstrap-probe", O_RDWR | O_CREAT);
	if (fd < 3 || write(fd, "Z", 1) != 1 || lseek(fd, 0, SEEK_SET) != 0
		|| read(fd, &byte, 1) != 1 || byte != 'Z' || close(fd) != 0)
		return 101;
	return 42;
}
