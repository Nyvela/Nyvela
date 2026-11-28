#ifndef NYVUTILS_H
#define NYVUTILS_H

#include <stdint.h>
#include <stddef.h>

bool ki64toa(int64_t i, char *buf, size_t max_len);
void* memcpy(void *dest, const void *src, size_t n);
void* memset(void *dest, const uint8_t val, size_t n);

size_t kstrlen(const char *s);
int kstrcmp(const char *a, const char *b);
int kstrncmp(const char *a, const char *b, size_t n);
char *kstrcpy(char *dst, const char *src);
char *kstrncpy(char *dst, const char *src, size_t n);
int kmemcmp(const void *a, const void *b, size_t n);

#endif // NYVUTILS_H
