#ifndef NYVD_H
#define NYVD_H

typedef struct uprocess {
  uint64_t pid;
  uint64_t as_handle;
  uint64_t main_thread;
} uprocess_t;

void _start(void);

#endif // NYVD_H
