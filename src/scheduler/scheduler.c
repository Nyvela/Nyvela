#include "../../include/nyvela/scheduler/scheduler.h"
#include "../../include/nyvela/drivers/video/console/console.h"
#include "../../include/nyvela/thread/thread.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/arch/x86_64/gdt.h"
#include "../../include/nyvela/arch/x86_64/asm/io.h"

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

void scheduler_tick(void) {
  thread_t *next = scheduler_next();

  if (next != current_thread) current_thread = next;
  
  tss.rsp0 = (uint64_t)current_thread->kernel_stack + 4096;

  switch_context(current_thread->context);
  __builtin_unreachable();  
}
