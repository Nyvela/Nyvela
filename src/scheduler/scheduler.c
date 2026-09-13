#include "../../include/nyvela/scheduler/scheduler.h"
#include "../../include/nyvela/drivers/video/console/console.h"
#include "../../include/nyvela/thread/thread.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/arch/x86_64/gdt.h"

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
  if (!current_thread || !ctx) return;

  // save_context() allocates a fresh context_t per timer tick.
  // Free the previous buffer so preemption doesn't leak a context per tick;
  // freed 168-byte blocks are reused by the next tick (heap has no coalescing).
  context_t *old = current_thread->context;
  current_thread->context = ctx;

  if (old && old != ctx) kfree(old);

  thread_t *next = scheduler_next();

  if (!next) {
    switch_context(ctx);
    return;
  }

  if (next == current_thread) {
    switch_context(ctx);
    return;
  }

  current_thread = next;

  if (next->kernel_stack) {
    tss_set_rsp0((uint64_t)next->kernel_stack + 4096);
  }

  switch_context(next->context);
}
