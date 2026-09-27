#ifndef NYVD_H
#define NYVD_H

typedef struct thread thread_t;

typedef struct uprocess {
  uint64_t pid;
  uint64_t as_handle;
  thread_t *main_thread;
} uprocess_t;

void _start(void);

#endif // NYVD_H
