#include "../../include/nyvela/scheduler/scheduler.h"
#include "../../include/nyvela/drivers/video/console/console.h"
#include "../../include/nyvela/thread/thread.h"

thread_t* scheduler_next() {
  if (threads_length == 0)
    return NULL;
  
  uint64_t current_index = 0;
  
  if (current_thread) {
    for (uint64_t i = 0; i < threads_length; i++) {
      if (threads[i] == current_thread) {
        current_index = i;
        break;
      }
    }
  }

  for (uint64_t i = 1; i <= threads_length; i++) {
    uint64_t index = (current_index + i) % threads_length;

    if (threads[index]->state == THREAD_READY) {
      return threads[index];
    }
  }

  return current_thread;
}

void scheduler_tick(context_t* ctx) {
  current_thread->context = ctx;

  thread_t *next = scheduler_next();

  if (next == current_thread) return;
  
  current_thread = next;
  switch_context(next->context);
}
