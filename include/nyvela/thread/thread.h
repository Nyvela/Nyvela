#ifndef NYVTHREAD_H
#define NYVTHREAD_H

#include <stdint.h>
#include "../arch/x86_64/context.h"

typedef enum thread_state_t {
  READY,
  RUNNING,
  BLOCKED,
  DEAD
} thread_state_t;

typedef struct thread_t {
  uint64_t tid;
  
  context_t context;
  void* kernel_stack;

  thread_state_t stage;
} thread_t

#endif // NYVTHREAD_H
