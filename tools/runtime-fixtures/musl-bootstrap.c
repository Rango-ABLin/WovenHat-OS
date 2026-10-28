#include <errno.h>
#include <fcntl.h>
#include <sched.h>
#include <signal.h>
#include <pthread.h>
#include <stdlib.h>
#include <stdint.h>
#include <sys/wait.h>
#include <unistd.h>

static char pipe_buffer[2048];
_Noreturn void _Exit(int);
void (*__wovenhat_signal_no_restart(int, void (*)(int)))(int);

static void note_sigpipe(int sig)
{
	_Exit(sig == SIGPIPE ? 42 : 99);
}

static void resume_after_signal(int sig)
{
	(void)sig;
}

static void *thread_probe(void *argument)
{
	char *values[16];
	if (!argument) return (void *)1;
	for (int i = 0; i < 16; ++i) {
		values[i] = malloc((size_t)(i + 1) * 8);
		if (!values[i]) return (void *)1;
		values[i][0] = 'T';
	}
	for (int i = 15; i >= 0; --i) free(values[i]);
	return (void *)42;
}

struct sync_probe {
	pthread_mutex_t mutex;
	pthread_cond_t condition;
	int ready;
};

static void *sync_thread(void *argument)
{
	struct sync_probe *probe = argument;
	if (pthread_mutex_lock(&probe->mutex) != 0)
		return (void *)1;
	probe->ready = 1;
	pthread_cond_signal(&probe->condition);
	pthread_mutex_unlock(&probe->mutex);
	return 0;
}

int main(int argc, char **argv, char **envp)
{
	char byte = 'x';
	char motd[24];
	char cwd[256];
	int fd;
	int duplicate;
	int replacement;
	int pipe_fd[2];
	int child_status;
	pid_t child;
	if (envp != argv + argc + 1)
		return 80;
	/* The loader supplies a bounded aux-vector after the empty envp. */
	unsigned long *aux = (unsigned long *)(argv + argc + 2);
	int saw_pagesz = 0, saw_uid = 0, saw_gid = 0, saw_random = 0;
	for (; aux[0] != 0; aux += 2) {
		if (aux[0] == 6 && aux[1] == 4096) saw_pagesz = 1;
		if (aux[0] == 11 && aux[1] == 1000) saw_uid = 1;
		if (aux[0] == 12 && aux[1] == 1000) saw_gid = 1;
		if (aux[0] == 25 && aux[1] != 0) {
			const unsigned char *random = (const unsigned char *)(uintptr_t)aux[1];
			for (int i = 0; i < 16; ++i)
				if (random[i] != 0) saw_random = 1;
		}
	}
	if (!saw_pagesz || !saw_uid || !saw_gid || !saw_random)
		return 95;

	if (getpid() < 0)
		return 89;
	if (getppid() < 0)
		return 90;
	if (getuid() == (uid_t)-1 || getgid() == (gid_t)-1)
		return 88;
	pthread_t thread;
	void *thread_result;
	if (pthread_create(&thread, 0, thread_probe, &byte) != 0
		|| pthread_join(thread, &thread_result) != 0
		|| thread_result != (void *)42)
		return 86;
	struct sync_probe probe = { PTHREAD_MUTEX_INITIALIZER, PTHREAD_COND_INITIALIZER, 0 };
	if (pthread_create(&thread, 0, sync_thread, &probe) != 0
		|| pthread_mutex_lock(&probe.mutex) != 0)
		return 85;
	while (!probe.ready && pthread_cond_wait(&probe.condition, &probe.mutex) == 0)
		;
	if (!probe.ready || pthread_mutex_unlock(&probe.mutex) != 0
		|| pthread_join(thread, 0) != 0)
		return 84;
	if (pthread_mutex_lock(&probe.mutex) != 0)
		return 83;
	struct timespec deadline = { 0, 1 };
	if (pthread_cond_timedwait(&probe.condition, &probe.mutex, &deadline) != ETIMEDOUT
		|| pthread_mutex_unlock(&probe.mutex) != 0)
		return 82;
	if (pthread_create(&thread, 0, thread_probe, &byte) != 0
		|| pthread_detach(thread) != 0 || sched_yield() != 0)
		return 81;
	if (!getcwd(cwd, sizeof cwd) || cwd[0] != '/')
		return 87;
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
	fd = open("/etc/motd", O_RDONLY);
	if (fd < 3 || read(fd, &byte, 1) != 1 || byte != 'W')
		return 102;
	duplicate = dup(fd);
	if (fd < 3 || duplicate < 3 || duplicate == fd || close(fd) != 0
		|| read(duplicate, &byte, 1) != 1 || byte != 'e' || close(duplicate) != 0)
		return 103;
	fd = open("/etc/motd", O_RDONLY);
	replacement = dup2(fd, fd + 1);
	if (fd < 3 || replacement != fd + 1 || close(fd) != 0
		|| read(replacement, &byte, 1) != 1 || byte != 'W' || close(replacement) != 0)
		return 104;
	if (pipe(pipe_fd) != 0 || write(pipe_fd[1], "P", 1) != 1
		|| read(pipe_fd[0], &byte, 1) != 1 || byte != 'P'
		|| close(pipe_fd[1]) != 0 || read(pipe_fd[0], &byte, 1) != 0
		|| close(pipe_fd[0]) != 0)
		return 105;
	/* A full pipe must block the child until the parent consumes capacity. */
	if (pipe(pipe_fd) != 0 || (child = fork()) < 0)
		return 106;
	if (child == 0) {
		if (close(pipe_fd[0]) != 0 || write(pipe_fd[1], pipe_buffer, 1024) != 1024
			|| write(pipe_fd[1], pipe_buffer, 1024) != 1024
			|| write(pipe_fd[1], "B", 1) != 1 || close(pipe_fd[1]) != 0)
			return 107;
		return 0;
	}
	if (close(pipe_fd[1]) != 0)
		return 108;
	if (sched_yield() != 0)
		return 114;
	if (read(pipe_fd[0], pipe_buffer, 1024) != 1024)
		return 115;
	if (read(pipe_fd[0], pipe_buffer, 1024) != 1024)
		return 116;
	if (sched_yield() != 0 || read(pipe_fd[0], &byte, 1) != 1 || byte != 'B')
		return 120;
	if (close(pipe_fd[0]) != 0)
		return 117;
	if (waitpid(child, &child_status, 0) != child)
		return 118;
	if (child_status != 0)
		return 119;
	/* With every reader closed, a write uses the default SIGPIPE path. */
	if (pipe(pipe_fd) != 0 || (child = fork()) < 0)
		return 109;
	if (child == 0) {
		if (close(pipe_fd[0]) != 0 || sched_yield() != 0)
			return 110;
		return write(pipe_fd[1], "S", 1) == -1 ? 111 : 112;
	}
	if (close(pipe_fd[0]) != 0 || close(pipe_fd[1]) != 0 || sched_yield() != 0
		|| waitpid(child, &child_status, 0) != child || !WIFSIGNALED(child_status)
		|| WTERMSIG(child_status) != SIGPIPE)
		return 113;
	/* Ignored and caught SIGPIPE both report EPIPE and preserve the child. */
	if (pipe(pipe_fd) != 0 || (child = fork()) < 0)
		return 121;
	if (child == 0) {
		if (signal(SIGPIPE, SIG_IGN) == SIG_ERR || close(pipe_fd[0]) != 0
			|| sched_yield() != 0)
			return 122;
		return write(pipe_fd[1], "I", 1) == -1 && errno == EPIPE ? 0 : 123;
	}
	if (close(pipe_fd[0]) != 0 || close(pipe_fd[1]) != 0 || sched_yield() != 0
		|| waitpid(child, &child_status, 0) != child || !WIFEXITED(child_status)
		|| WEXITSTATUS(child_status) != 0)
		return 124;
	if (pipe(pipe_fd) != 0 || (child = fork()) < 0)
		return 125;
	if (child == 0) {
		if (signal(SIGPIPE, note_sigpipe) == SIG_ERR || close(pipe_fd[0]) != 0
			|| sched_yield() != 0)
			return 126;
		return write(pipe_fd[1], "C", 1) == -1 && errno == EPIPE ? 127 : 129;
	}
	if (close(pipe_fd[0]) != 0 || close(pipe_fd[1]) != 0 || sched_yield() != 0
		|| waitpid(child, &child_status, 0) != child || !WIFEXITED(child_status)
		|| WEXITSTATUS(child_status) != 42)
		return 128;
	/* A caught non-terminal signal wakes a reader and returns EINTR. */
	if (pipe(pipe_fd) != 0 || (child = fork()) < 0)
		return 130;
	if (child == 0) {
		if (__wovenhat_signal_no_restart(SIGUSR1, resume_after_signal) == SIG_ERR || close(pipe_fd[1]) != 0)
			return 131;
		return read(pipe_fd[0], &byte, 1) == -1 && errno == EINTR ? 0 : 132;
	}
	if (close(pipe_fd[0]) != 0 || sched_yield() != 0 || kill(child, SIGUSR1) != 0
		|| waitpid(child, &child_status, 0) != child || !WIFEXITED(child_status)
		|| WEXITSTATUS(child_status) != 0 || close(pipe_fd[1]) != 0)
		return 133;
	/* signal() requests restart: after SIGUSR1, data completes the same read. */
	if (pipe(pipe_fd) != 0 || (child = fork()) < 0)
		return 134;
	if (child == 0) {
		if (signal(SIGUSR1, resume_after_signal) == SIG_ERR || close(pipe_fd[1]) != 0)
			return 135;
		return read(pipe_fd[0], &byte, 1) == 1 && byte == 'R' ? 0 : 136;
	}
	if (close(pipe_fd[0]) != 0 || sched_yield() != 0 || kill(child, SIGUSR1) != 0
		|| write(pipe_fd[1], "R", 1) != 1 || waitpid(child, &child_status, 0) != child
		|| !WIFEXITED(child_status) || WEXITSTATUS(child_status) != 0
		|| close(pipe_fd[1]) != 0)
		return 137;
	return 42;
}
