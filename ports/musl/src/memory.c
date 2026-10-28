/* Minimal freestanding memory/string primitives required by the selected
 * musl objects.  They are independent of the runtime allocator. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

void *memset(void *destination, int value, size_t length)
{
	unsigned char *bytes = destination;
	for (size_t index = 0; index < length; ++index)
		bytes[index] = (unsigned char)value;
	return destination;
}

void *memcpy(void *destination, const void *source, size_t length)
{
	unsigned char *out = destination;
	const unsigned char *in = source;
	for (size_t index = 0; index < length; ++index)
		out[index] = in[index];
	return destination;
}

char *strdup(const char *source)
{
	if (!source)
		return 0;
	size_t length = 0;
	while (source[length])
		++length;
	char *copy = malloc(length + 1);
	if (!copy)
		return 0;
	memcpy(copy, source, length + 1);
	return copy;
}
