#ifndef NYVPROCESS_H
#define NYVPROCESS_H

#include "../mm/vmm.h"

typedef struct thread_t thread_t;

typedef struct process_t {
  uint64_t cr3;
  uint64_t pid;

  vm_area_t *vma;
  thread_t **threads;
} process_t;

extern process_t **processes;
extern uint64_t processes_length;
extern uint64_t processes_cap;

extern process_t *current_process;

process_t* spawn_process(void (*entry)(void));
void free_process(process_t* process);

#endif // NYVPROCESS_H
