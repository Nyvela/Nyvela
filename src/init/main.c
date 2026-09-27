#include <nyvela/user/syslib.h>
#include "../../include/nyvela/init/init.h"
#include "../../include/nyvela/mm/vmm.h"

void new_thread(void) {
  for (;;) {
    __asm__ volatile ("pause");
  }
}

void _start(void) {
  uint64_t as = sys_call(SYS_AS_CREATE, 0, 0, 0, 0);

  if (!as) {
    char buf[] = "Failed to allocate address space.";
    sys_call(SYS_WRITE, 1, (uint64_t)buf, sizeof(buf), 0);
    for (;;);
  }
  
  uint64_t stack_phys = (uint64_t)sys_call(SYS_ALLOC_PAGE, 0, 0, 0, 0);

  if (!stack_phys) {
    char buf[] = "Failed to allocate stack for process";
    sys_call(SYS_WRITE, 1, (uint64_t)buf, sizeof(buf), 0);
    for (;;);
  }

  if (!sys_call(SYS_AS_MAP, as, USER_STACK_PAGE, stack_phys, 0x07)) {
    char buf[] = "Failed to map process stack.";
    sys_call(SYS_WRITE, 1, (uint64_t)buf, sizeof(buf), 0);
    for (;;);
  }

  uint64_t thread_handle = sys_call(SYS_CREATE_THREAD, (uint64_t)new_thread, as, (uint64_t)USER_STACK_PAGE + 4096, 0);
  
  if (!thread_handle) {
    char buf[] = "Failed to create thread.";
    sys_call(SYS_WRITE, 1, (uint64_t)buf, sizeof(buf), 0);
    sys_exit(1);
  }

  char buf[] = "Created thread!";
  sys_call(SYS_WRITE, 1, (uint64_t)buf, sizeof(buf), 0);
    
  for (;;);

  sys_exit(0);
}
