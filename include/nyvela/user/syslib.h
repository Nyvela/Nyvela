#ifndef NYV_SYSLIB_H
#define NYV_SYSLIB_H

// Nyvela user-space SDK (freestanding, no libc).
// Provides raw syscall wrappers (int $0x80) and tiny string helpers.
// User programs link as flat binaries for a fixed base address
// (see src/user/ld/*.ld) and start at _start with a fresh stack.
//
// Syscall numbers must match include/nyvela/syscall/syscall.h.

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#define SYS_EXIT      0ULL
#define SYS_WRITE     1ULL
#define SYS_YIELD     2ULL
#define SYS_READ      3ULL

#define SYS_FS_CREATE 10ULL
#define SYS_FS_WRITE  11ULL
#define SYS_FS_READ   12ULL
#define SYS_FS_LIST   13ULL

#define SYS_EXEC      20ULL

static inline uint64_t sys_call(uint64_t nr, uint64_t a1, uint64_t a2,
                                uint64_t a3, uint64_t a4) {
  register uint64_t r10 __asm__("r10") = a4;
  uint64_t ret;

  // The kernel stub preserves every register except rax (the return).
  __asm__ volatile ("int $0x80"
                    : "=a"(ret)
                    : "a"(nr), "D"(a1), "S"(a2), "d"(a3), "r"(r10)
                    : "memory");

  return ret;
}

static inline void sys_exit(int64_t code) {
  sys_call(SYS_EXIT, (uint64_t)code, 0, 0, 0);

  for (;;) {
  }
}

static inline int64_t sys_write(uint64_t fd, const void *buf, uint64_t len) {
  return (int64_t)sys_call(SYS_WRITE, fd, (uint64_t)buf, len, 0);
}

static inline int64_t sys_read(uint64_t fd, void *buf, uint64_t len) {
  return (int64_t)sys_call(SYS_READ, fd, (uint64_t)buf, len, 0);
}

static inline int64_t sys_yield(void) {
  return (int64_t)sys_call(SYS_YIELD, 0, 0, 0, 0);
}

static inline int64_t sys_fs_create(const char *path, uint64_t is_dir) {
  return (int64_t)sys_call(SYS_FS_CREATE, (uint64_t)path, is_dir, 0, 0);
}

static inline int64_t sys_fs_write(const char *path, const void *buf,
                                   uint64_t len, uint64_t offset) {
  return (int64_t)sys_call(SYS_FS_WRITE, (uint64_t)path, (uint64_t)buf,
                           len, offset);
}

static inline int64_t sys_fs_read(const char *path, void *buf, uint64_t len,
                                  uint64_t offset) {
  return (int64_t)sys_call(SYS_FS_READ, (uint64_t)path, (uint64_t)buf,
                           len, offset);
}

static inline int64_t sys_fs_list(const char *path, char *buf, uint64_t len) {
  return (int64_t)sys_call(SYS_FS_LIST, (uint64_t)path, (uint64_t)buf, len, 0);
}

static inline int64_t sys_exec(const char *path) {
  return (int64_t)sys_call(SYS_EXEC, (uint64_t)path, 0, 0, 0);
}

// --- tiny string helpers (in-header so every program gets them) ---

static inline size_t ustrlen(const char *s) {
  size_t n = 0;

  while (s[n]) n++;

  return n;
}

static inline int ustrcmp(const char *a, const char *b) {
  while (*a && *a == *b) {
    a++;
    b++;
  }

  return (int)(unsigned char)*a - (int)(unsigned char)*b;
}

static inline int ustrncmp(const char *a, const char *b, size_t n) {
  for (size_t i = 0; i < n; i++) {
    unsigned char ca = (unsigned char)a[i];
    unsigned char cb = (unsigned char)b[i];

    if (ca != cb) return (int)ca - (int)cb;
    if (!ca) return 0;
  }

  return 0;
}

static inline void *umemcpy(void *dst, const void *src, size_t n) {
  uint8_t *d = dst;
  const uint8_t *s = src;

  for (size_t i = 0; i < n; i++) d[i] = s[i];

  return dst;
}

static inline void *umemset(void *dst, uint8_t v, size_t n) {
  uint8_t *d = dst;

  for (size_t i = 0; i < n; i++) d[i] = v;

  return d;
}

// Signed 64-bit to decimal. Returns chars written (excl. NUL).
static inline size_t uitoa(int64_t val, char *buf, size_t cap) {
  if (cap == 0) return 0;

  size_t pos = 0;
  bool neg = val < 0;
  uint64_t u = neg ? (uint64_t)(-(val + 1)) + 1ULL : (uint64_t)val;

  char tmp[20];
  size_t tlen = 0;

  do {
    tmp[tlen++] = (char)('0' + u % 10);
    u /= 10;
  } while (u && tlen < sizeof(tmp));

  if (neg && pos + 1 < cap) buf[pos++] = '-';

  while (tlen && pos + 1 < cap) buf[pos++] = tmp[--tlen];

  buf[pos] = '\0';

  return pos;
}

#endif // NYV_SYSLIB_H
