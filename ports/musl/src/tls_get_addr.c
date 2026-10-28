/* General-dynamic TLS resolver for the bounded WovenHat ELF loader. */
#include <stddef.h>
#include <stdint.h>
#include <wovenhat/syscall.h>

struct wovenhat_tls_index {
	uintptr_t ti_module;
	uintptr_t ti_offset;
};

void *__tls_get_addr(const struct wovenhat_tls_index *index)
{
	if (!index)
		return 0;
	long address = wh_syscall2(WH_SYS_DYNAMIC_TLS_ADDR,
		(long)index->ti_module, (long)index->ti_offset);
	return address < 0 ? 0 : (void *)(uintptr_t)address;
}
