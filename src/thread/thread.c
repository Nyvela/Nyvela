#include "../../include/nyvela/thread/thread.h"
#include "../../include/nyvela/arch/x86_64/gdt.h"
#include "../../include/nyvela/arch/x86_64/asm/cpu.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/lib/utils.h"
#include "../../include/nyvela/mm/vmm.h"
#include "../../include/nyvela/process/process.h"

extern uint8_t* kernel_end;

thread_t **threads;
thread_t *current_thread;

uint64_t threads_length = 0;
uint64_t threads_cap = 0;
uint64_t next_thread_id = 1;

thread_t* allocate_thread() {
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

  
  void *kernel_stack = NULL;
  
  if (KERNEL_STACK_SIZE_IN_PAGES == 1) { 
    kernel_stack = kpalloc_top();
  } else {
    kernel_stack = kpalloc_contiguous(KERNEL_STACK_SIZE_IN_PAGES);
    
    if (kernel_stack && (uint64_t)kernel_stack < 0x600000) {
      kpfree_contiguous(kernel_stack, KERNEL_STACK_SIZE_IN_PAGES);
      kernel_stack = NULL;
      
      for (uint64_t p = FRAME_COUNT; p-- > (((uint64_t)&kernel_end + 4095) / 4096) + KERNEL_STACK_SIZE_IN_PAGES;) {
        (void)p; 
        break;
      }
      
      kernel_stack = kpalloc_contiguous(KERNEL_STACK_SIZE_IN_PAGES);
    }
  }

  if (!kernel_stack) {
    kfree(thread);
    return NULL;
  }

  context_t *context = kmalloc(sizeof(context_t));

  if (!context) {
    kfree(thread);
    kpfree_contiguous(kernel_stack, KERNEL_STACK_SIZE_IN_PAGES);
    return NULL;
  }

  memset(context, 0, sizeof(context_t));

  context->rsp = (uint64_t)kernel_stack + 4096 * KERNEL_STACK_SIZE_IN_PAGES;

  thread->context = context;
  thread->kernel_stack = kernel_stack;
  thread->process = current_process;

  return thread;
}

void free_thread(thread_t* thread) {
  if (!thread) return;

  if (thread->kernel_stack) {
    kpfree_contiguous(thread->kernel_stack, KERNEL_STACK_SIZE_IN_PAGES);
  }

  if (thread->context) {
    kfree(thread->context);
  }

  kfree(thread);
}

thread_t* spawn_thread(void (*entry)(void)) {
  if (!entry) return NULL;

  thread_t* thread = allocate_thread();

  if (!thread) return NULL;

  thread->context->rip = (uint64_t)entry;
  thread->context->rflags = 0x202;
  thread->context->cs = GDT_KCODE_LM_SEGMENT;
  thread->context->ss = GDT_KDATA_SEGMENT;

  thread->tid = next_thread_id++;
  thread->state = THREAD_READY;
  thread->exit_code = 0;

  if (threads_length >= threads_cap) {
    uint64_t new_cap = threads_cap * 2;
    thread_t **tmp = (thread_t**)kmalloc(sizeof(thread_t*) * new_cap);

    if (!tmp) {
      free_thread(thread);
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
