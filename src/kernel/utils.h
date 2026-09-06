#ifndef NYVUTILS_H
#define NYVUTILS_H

#include <stdint.h>
#include <stddef.h>

bool ki64toa(int64_t i, char *buf, size_t max_len);
void* memcpy(void *dest, const void *src, size_t n);
void* memset(void *dest, const uint8_t val, size_t n);

#endif // NYVUTILS_H
