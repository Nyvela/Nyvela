#ifndef NYVTHREAD_H
#define NYVTHREAD_H

#include <stdint.h>
#include "../arch/x86_64/context.h"

#define KERNEL_STACK_SIZE_IN_PAGES 1

typedef enum thread_state_t {
  THREAD_READY,
  THREAD_RUNNING,
  THREAD_BLOCKED,
  THREAD_DEAD
} thread_state_t;

typedef struct thread_t {
  uint64_t tid;
  
  context_t* context;
  void* kernel_stack;

  thread_state_t state;
} thread_t;

extern thread_t **threads;
extern thread_t *current_thread;

extern uint64_t threads_length;

extern uint64_t next_thread_id;

thread_t* spawn_thread(void (*entry)(void));

#endif // NYVTHREAD_H
