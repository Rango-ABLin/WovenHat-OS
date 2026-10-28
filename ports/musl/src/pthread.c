/* Bounded pthread core over WovenHat's generation-safe thread ABI. */
#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include <stddef.h>
#include <stdint.h>
#include <wovenhat/syscall.h>

#define MAX_PTHREADS 8
struct slot { void *(*start)(void *); void *arg; pthread_t handle; int busy; int detached; int finished; };
static struct slot slots[MAX_PTHREADS];
static volatile int slots_lock;
static void lock(void) { while (__sync_lock_test_and_set(&slots_lock, 1)) sched_yield(); }
static void unlock(void) { __sync_lock_release(&slots_lock); }
static void run_slot(size_t index)
{
	void *result = slots[index].start(slots[index].arg);
	lock();
	slots[index].finished = 1;
	if (slots[index].detached)
		slots[index].busy = 0;
	unlock();
	wh_syscall1(WH_SYS_THREAD_EXIT, (long)(intptr_t)result);
	for (;;) ;
}
#define TRAMPOLINE(n) static void trampoline##n(void) { run_slot(n); }
TRAMPOLINE(0) TRAMPOLINE(1) TRAMPOLINE(2) TRAMPOLINE(3)
TRAMPOLINE(4) TRAMPOLINE(5) TRAMPOLINE(6) TRAMPOLINE(7)
static void (*const trampolines[MAX_PTHREADS])(void) = { trampoline0, trampoline1, trampoline2, trampoline3, trampoline4, trampoline5, trampoline6, trampoline7 };
int pthread_create(pthread_t *thread, const pthread_attr_t *attr, void *(*start)(void *), void *arg)
{
	(void)attr;
	if (!thread || !start) return EINVAL;
	lock(); size_t index;
	for (index = 0; index < MAX_PTHREADS && slots[index].busy; ++index) ;
	if (index == MAX_PTHREADS) { unlock(); return EAGAIN; }
	slots[index].start = start; slots[index].arg = arg; slots[index].busy = 1;
	long result = wh_syscall2(WH_SYS_THREAD_CREATE, (long)(uintptr_t)trampolines[index], 64 * 1024);
	if (result < 0) { slots[index].busy = 0; unlock(); return EAGAIN; }
	slots[index].handle = (pthread_t)(unsigned long)result;
	slots[index].detached = 0;
	slots[index].finished = 0;
	*thread = slots[index].handle;
	unlock(); return 0;
}
int pthread_join(pthread_t thread, void **result)
{
	long status = wh_syscall1(WH_SYS_THREAD_JOIN, (long)(unsigned long)thread);
	if (status == -1) return EINVAL;
	if (status == -2) return EBUSY;
	if (status < 0) return EINVAL;
	if (result) *result = (void *)(intptr_t)status;
	lock();
	for (size_t i = 0; i < MAX_PTHREADS; ++i)
		if (slots[i].busy && slots[i].handle == thread) {
			if (slots[i].detached) { unlock(); return EINVAL; }
			slots[i].busy = 0;
			break;
		}
	unlock(); return 0;
}
int pthread_detach(pthread_t thread)
{
	lock();
	for (size_t i = 0; i < MAX_PTHREADS; ++i)
		if (slots[i].busy && slots[i].handle == thread) {
			if (slots[i].detached) { unlock(); return EINVAL; }
			slots[i].detached = 1;
			if (slots[i].finished) slots[i].busy = 0;
			unlock();
			return 0;
		}
	unlock();
	return ESRCH;
}
pthread_t pthread_self(void) { return (pthread_t)(unsigned long)wh_syscall0(WH_SYS_THREAD_SELF); }
int pthread_cancel(pthread_t thread)
{
	return wh_syscall2(WH_SYS_THREAD_CANCEL, (long)(unsigned long)thread, 0) < 0 ? EINVAL : 0;
}

/* The bootstrap profile uses the first musl lock word.  It is deliberately
 * non-recursive and process-private; no kernel lock is held while yielding. */
int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attr)
{
	(void)attr;
	if (!mutex) return EINVAL;
	mutex->__u.__vi[0] = 0;
	return 0;
}
int pthread_mutex_destroy(pthread_mutex_t *mutex)
{
	if (!mutex || mutex->__u.__vi[0]) return EBUSY;
	return 0;
}
int pthread_mutex_trylock(pthread_mutex_t *mutex)
{
	if (!mutex) return EINVAL;
	return __sync_bool_compare_and_swap(&mutex->__u.__vi[0], 0, 1) ? 0 : EBUSY;
}
int pthread_mutex_lock(pthread_mutex_t *mutex)
{
	if (!mutex) return EINVAL;
	while (!__sync_bool_compare_and_swap(&mutex->__u.__vi[0], 0, 1))
		sched_yield();
	return 0;
}
int pthread_mutex_unlock(pthread_mutex_t *mutex)
{
	if (!mutex || !mutex->__u.__vi[0]) return EINVAL;
	__sync_lock_release(&mutex->__u.__vi[0]);
	return 0;
}
int pthread_mutex_consistent(pthread_mutex_t *mutex)
{
	(void)mutex;
	return ENOTSUP;
}

/* Condition waits use a sequence counter.  The caller's mutex is released
 * before yielding and reacquired before return, preventing lost wakeups for
 * the normal predicate-loop pattern. Timed waits remain explicitly outside
 * this bounded profile. */
int pthread_cond_init(pthread_cond_t *condition, const pthread_condattr_t *attr)
{
	(void)attr;
	if (!condition) return EINVAL;
	condition->__u.__vi[0] = 0;
	return 0;
}
int pthread_cond_destroy(pthread_cond_t *condition)
{
	return condition ? 0 : EINVAL;
}
int pthread_cond_signal(pthread_cond_t *condition)
{
	if (!condition) return EINVAL;
	__sync_add_and_fetch(&condition->__u.__vi[0], 1);
	return 0;
}
int pthread_cond_broadcast(pthread_cond_t *condition)
{
	return pthread_cond_signal(condition);
}
int pthread_cond_wait(pthread_cond_t *condition, pthread_mutex_t *mutex)
{
	if (!condition || !mutex) return EINVAL;
	int sequence = condition->__u.__vi[0];
	if (pthread_mutex_unlock(mutex)) return EINVAL;
	while (condition->__u.__vi[0] == sequence) sched_yield();
	return pthread_mutex_lock(mutex);
}
int pthread_cond_timedwait(pthread_cond_t *condition, pthread_mutex_t *mutex,
	const struct timespec *timeout)
{
	if (!condition || !mutex || !timeout || timeout->tv_nsec < 0 || timeout->tv_nsec >= 1000000000L)
		return EINVAL;
	int sequence = condition->__u.__vi[0];
	if (pthread_mutex_unlock(mutex)) return EINVAL;
	/* Bounded static profile: yield for a finite slice, then report timeout.
	 * A kernel clock-backed wait remains a later integration increment. */
	for (unsigned long spins = 0; spins < 4096 && condition->__u.__vi[0] == sequence; ++spins)
		sched_yield();
	if (condition->__u.__vi[0] == sequence) {
		(void)pthread_mutex_lock(mutex);
		return ETIMEDOUT;
	}
	return pthread_mutex_lock(mutex);
}
