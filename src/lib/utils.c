#include "../../include/nyvela/lib/utils.h"

bool ki64toa(int64_t i, char *buf, size_t max_len) {
  size_t pos = 0;
  uint64_t value;
  
  if (max_len == 0) {
    return false;
  }

  if (i < 0) {
    if (pos >= max_len - 1) return false;

    buf[pos++] = '-';
    value = -(uint64_t)i;
  } else {
    value = (uint64_t)i;
  }

  if (value == 0) {
    if (pos >= max_len - 1) return false;

    buf[pos++] = '0';
  } else {
    while (value != 0) {
      if (pos >= max_len - 1) return false;
    
      uint8_t d = value % 10;
      buf[pos++] = '0' + d;
      value /= 10;
    }
  }
  
  buf[pos] = '\0';
  
  size_t start = (i < 0) ? 1 : 0;

  for (size_t a = start, b = pos - 1; a < b; a++, b--) {
    char tmp = buf[a];
    buf[a] = buf[b];
    buf[b] = tmp;
  }

  return true;
}

void* memcpy(void* dest, const void* src, size_t n) {
  uint8_t *d = dest;
  const uint8_t *s = src;

  for (size_t i = 0; i < n; i++) {
    d[i] = s[i];
  }

  return dest;
} 

void* memset(void* dest, const uint8_t val, size_t n) {
  uint8_t *d = dest;

  for (size_t i = 0; i < n; i++) {
    d[i] = val;
  }

  return dest;
}

size_t kstrlen(const char *s) {
  size_t n = 0;

  while (s && s[n]) n++;

  return n;
}

int kstrcmp(const char *a, const char *b) {
  while (*a && *b && *a == *b) {
    a++;
    b++;
  }

  return (int)(uint8_t)*a - (int)(uint8_t)*b;
}

int kstrncmp(const char *a, const char *b, size_t n) {
  for (size_t i = 0; i < n; i++) {
    uint8_t ca = (uint8_t)a[i];
    uint8_t cb = (uint8_t)b[i];

    if (ca != cb) return (int)ca - (int)cb;
    if (ca == '\0') return 0;
  }

  return 0;
}

char *kstrcpy(char *dst, const char *src) {
  size_t i = 0;

  while (src[i]) {
    dst[i] = src[i];
    i++;
  }

  dst[i] = '\0';

  return dst;
}

char *kstrncpy(char *dst, const char *src, size_t n) {
  size_t i = 0;

  for (; i < n && src[i]; i++) {
    dst[i] = src[i];
  }

  for (; i < n; i++) {
    dst[i] = '\0';
  }

  return dst;
}

int kmemcmp(const void *a, const void *b, size_t n) {
  const uint8_t *pa = a;
  const uint8_t *pb = b;

  for (size_t i = 0; i < n; i++) {
    if (pa[i] != pb[i]) return (int)pa[i] - (int)pb[i];
  }

  return 0;
} 
