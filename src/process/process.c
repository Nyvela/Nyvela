#include "../../include/nyvela/process/process.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/mm/vmm.h"
#include "../../include/nyvela/thread/thread.h"

#include <stddef.h>

process_t **processes = NULL;
uint64_t processes_length = 0;
uint64_t processes_cap = 4;

process_t *current_process = 0;

uint64_t current_pid = 0;

void free_process(process_t* process) {
  kvmm_free_user_pml4(process->cr3);
  kfree(process->vma);
  kfree(process);
}

process_t* spawn_process(void (*entry)(void)) {
  if (!entry) 
    return NULL;

  if (!processes) {
    processes = kmalloc(sizeof(process_t*) * processes_cap);

    if (!processes) {
      return NULL;
    }
  }

  process_t *process = kmalloc(sizeof(process_t));

  if (!process)
    return NULL;
  
  process->cr3 = kvmm_create_user_pml4();
  process->pid = current_pid++;
  process->vma = kmalloc(sizeof(vm_area_t));

  if (!process->vma) {
    kvmm_free_user_pml4(process->cr3);
    kfree(process);

    return NULL;
  }

  process->threads = kmalloc(sizeof(thread_t*));

  if (!process->threads) { 
    kvmm_free_user_pml4(process->cr3);
    kfree(process->vma);
    kfree(process);

    return NULL;
  }
  
  current_process = process;
  process->threads[0] = spawn_thread(entry);

  if (processes_length >= processes_cap) {
    uint64_t new_cap = processes_cap * 2;
    void *tmp = krealloc(processes, sizeof(process_t*) * new_cap);

    if (!tmp) {
      kvmm_free_user_pml4(process->cr3);
      kfree(process->vma);
      kfree(process);

      return NULL;
    }

    processes = tmp;
  }

  processes[processes_length++] = process;
  return process;
}
