/* Bounded allocator over WovenHat's checked anonymous mmap ABI. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wovenhat/syscall.h>

#define ARENA_SIZE (64UL * 1024UL)
#define MAX_BLOCKS 64
struct block { unsigned char *base; size_t size; int free; };
static struct block blocks[MAX_BLOCKS];
static volatile int allocator_lock;
static void lock(void) { while (__sync_lock_test_and_set(&allocator_lock, 1)) ; }
static void unlock(void) { __sync_lock_release(&allocator_lock); }
static size_t aligned_size(size_t size)
{
	if (!size || size > SIZE_MAX - 15) return 0;
	return (size + 15) & ~(size_t)15;
}
void *malloc(size_t size)
{
	size = aligned_size(size);
	if (!size || size > ARENA_SIZE) return 0;
	lock();
	for (size_t i = 0; i < MAX_BLOCKS; ++i) {
		if (blocks[i].free && blocks[i].size >= size) {
			blocks[i].free = 0; unlock(); return blocks[i].base;
		}
	}
	for (size_t i = 0; i < MAX_BLOCKS; ++i) if (!blocks[i].base) {
		long result = wh_syscall2(WH_SYS_MMAP, ARENA_SIZE, 1);
		if (result < 0) { unlock(); return 0; }
		blocks[i].base = (unsigned char *)(uintptr_t)result;
		blocks[i].size = ARENA_SIZE; blocks[i].free = 0;
		if (size < ARENA_SIZE && i + 1 < MAX_BLOCKS) {
			blocks[i + 1].base = blocks[i].base + size;
			blocks[i + 1].size = ARENA_SIZE - size;
			blocks[i + 1].free = 1;
		}
		unlock(); return blocks[i].base;
	}
	unlock(); return 0;
}
void free(void *ptr)
{
	if (!ptr) return;
	lock();
	for (size_t i = 0; i < MAX_BLOCKS; ++i) if (blocks[i].base == ptr) { blocks[i].free = 1; break; }
	unlock();
}
void *calloc(size_t count, size_t size)
{
	if (count && size > SIZE_MAX / count) return 0;
	void *result = malloc(count * size);
	if (result) memset(result, 0, count * size);
	return result;
}
void *realloc(void *ptr, size_t size)
{
	if (!ptr) return malloc(size);
	if (!size) { free(ptr); return 0; }
	lock(); size_t old = 0;
	for (size_t i = 0; i < MAX_BLOCKS; ++i) if (blocks[i].base == ptr) old = blocks[i].size;
	unlock();
	if (!old) return 0;
	void *result = malloc(size);
	if (result) { memcpy(result, ptr, old < size ? old : size); free(ptr); }
	return result;
}
