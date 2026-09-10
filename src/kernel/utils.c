#include "../../include/kernel/utils.h"

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
