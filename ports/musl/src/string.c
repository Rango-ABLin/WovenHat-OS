/* Minimal freestanding string primitives required by the bootstrap runtime. */
#include <stddef.h>

void *memset(void *destination, int value, size_t size)
{
	unsigned char *bytes = destination;
	for (size_t index = 0; index < size; ++index)
		bytes[index] = (unsigned char)value;
	return destination;
}

void *memcpy(void *destination, const void *source, size_t size)
{
	unsigned char *out = destination;
	const unsigned char *in = source;
	for (size_t index = 0; index < size; ++index)
		out[index] = in[index];
	return destination;
}
