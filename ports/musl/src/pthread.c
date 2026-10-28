/* Bounded pthread core over WovenHat's generation-safe thread ABI. */
#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include <stddef.h>
#include <stdint.h>
#include <wovenhat/syscall.h>

#define MAX_PTHREADS 8
struct slot { void *(*start)(void *); void *arg; pthread_t handle; int busy; };
static struct slot slots[MAX_PTHREADS];
static volatile int slots_lock;
static void lock(void) { while (__sync_lock_test_and_set(&slots_lock, 1)) sched_yield(); }
static void unlock(void) { __sync_lock_release(&slots_lock); }
static void run_slot(size_t index)
{
	void *result = slots[index].start(slots[index].arg);
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
	slots[index].handle = (pthread_t)(unsigned long)result; *thread = slots[index].handle;
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
	for (size_t i = 0; i < MAX_PTHREADS; ++i) if (slots[i].busy && slots[i].handle == thread) { slots[i].busy = 0; break; }
	unlock(); return 0;
}
pthread_t pthread_self(void) { return (pthread_t)(unsigned long)wh_syscall0(WH_SYS_THREAD_SELF); }
int pthread_cancel(pthread_t thread)
{
	return wh_syscall2(WH_SYS_THREAD_CANCEL, (long)(unsigned long)thread, 0) < 0 ? EINVAL : 0;
}
