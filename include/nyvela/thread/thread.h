#ifndef NYVTHREAD_H
#define NYVTHREAD_H

#include <stdint.h>
#include <stddef.h>

#include "../arch/x86_64/context.h"
#include "../process/process.h"

#define KERNEL_STACK_SIZE_IN_PAGES 2

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
  process_t* process; 

  int64_t exit_code;
} thread_t;

_Static_assert(offsetof(thread_t, tid) == 0, "tid offset");
_Static_assert(offsetof(thread_t, context) == 8, "context offset");
_Static_assert(offsetof(thread_t, kernel_stack) == 16, "kernel_stack offset");
_Static_assert(offsetof(thread_t, state) == 24, "state offset");
_Static_assert(offsetof(thread_t, process) == 32, "process offset");
_Static_assert(offsetof(thread_t, exit_code) == 40, "exit_code offset");

_Static_assert(sizeof(thread_t) == 48, "thread_t has unexpected size");

extern thread_t **threads;
extern thread_t *current_thread;

extern uint64_t threads_length;

extern uint64_t next_thread_id;

thread_t* spawn_thread(void (*entry)(void));
void free_thread(thread_t* thread);

#endif // NYVTHREAD_H
