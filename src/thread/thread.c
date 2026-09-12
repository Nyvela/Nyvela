#include "../../include/nyvela/thread/thread.h"
#include "../../include/nyvela/arch/x86_64/gdt.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/lib/utils.h"

thread_t **threads;
thread_t *current_thread;

uint64_t threads_length = 0;
uint64_t threads_cap = 0;
uint64_t next_thread_id = 1;

thread_t* spawn_thread(void (*entry)(void)) {
  if (!entry) return NULL;
  
  if (threads_cap == 0) {
    threads = kmalloc(sizeof(thread_t*) * 8);
    
    if (!threads) {
      return NULL;
    }

    threads_cap = 8;
  }

  thread_t *thread = kmalloc(sizeof(thread_t));

  if (!thread) {
    return NULL;
  }

  void *kernel_stack = kpalloc();

  if (!kernel_stack) {
    kfree(thread);
    return NULL;
  }

  context_t *context = kmalloc(sizeof(context_t));

  if (!context) {
    kfree(thread);
    kpfree(kernel_stack);
    return NULL;
  }

  memset(context, 0, sizeof(context_t));
  
  uint64_t cr3;
  
  __asm__ volatile (
    "mov %%cr3, %0" : "=r"(cr3)
  );

  context->rip = (uint64_t)entry;
  context->rsp = (uint64_t)kernel_stack + 4096;
  context->rflags = 0x202;
  context->cs = GDT_KCODE_LM_SEGMENT;
  context->ss = GDT_KDATA_SEGMENT;
  context->cr3 = cr3;

  thread->tid = next_thread_id++;
  thread->context = context;
  thread->kernel_stack = kernel_stack;
  thread->state = THREAD_READY;

  if (threads_length >= threads_cap) {
    uint64_t new_cap = threads_cap * 2;
    thread_t **tmp = (thread_t**)kmalloc(sizeof(thread_t*) * new_cap);

    if (!tmp) {
      kpfree(kernel_stack);
      kfree(context);
      kfree(thread);

      return NULL;
    }

    for (uint64_t i = 0; i < threads_length; i++) {
      tmp[i] = threads[i];
    }

    kfree(threads);
    threads = tmp;
    threads_cap = new_cap;
  }

  threads[threads_length++] = thread;

  return thread;
}
