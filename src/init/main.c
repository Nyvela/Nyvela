#include <nyvela/user/syslib.h>
#include "../../include/nyvela/init/init.h"
#include "../../include/nyvela/mm/vmm.h"

#define SPAWN_READ_CHUNK 2048ULL
#define SPAWN_MAX_PAGES  8ULL

uint64_t next_pid = 1;
static uproc_table_t uprocess_table = {0};

static bool load_program(const char *path, uint64_t *stage, uint64_t size) {
  for (uint64_t pos = 0; pos < size; pos += SPAWN_READ_CHUNK) {
    uint64_t want = size - pos;

    if (want > SPAWN_READ_CHUNK) {
      want = SPAWN_READ_CHUNK;
    }

    uint64_t buf = stage[pos / 4096] + (pos % 4096);
    int64_t got = (int64_t)sys_call(SYS_FS_READ, (uint64_t)path, buf, want, pos);

    if (got != (int64_t)want) {
      return false;
    }
  }

  return true;
}

uprocess_t *spawn_process(const char *path) {
  int64_t sz = (int64_t)sys_call(SYS_FS_SIZE, (uint64_t)path, 0, 0, 0);

  if (sz < 4) {
    return NULL;
  }

  uint64_t size = (uint64_t)sz;

  if (size > SPAWN_MAX_PAGES * 4096ULL) {
    return NULL;
  }

  uint64_t pages = (size + 4095) / 4096;

  uprocess_t *proc = malloc(sizeof(uprocess_t));

  if (!proc) {
    return NULL;
  }

  if (uprocess_table.size >= uprocess_table.cap) {
    uint64_t new_cap = uprocess_table.cap * 2;
    void *tmp = realloc(uprocess_table.processes, sizeof(uprocess_t*) * new_cap);

    if (!tmp) {
      free(proc);
      return NULL;
    }

    uprocess_table.processes = tmp;
    uprocess_table.cap = new_cap;
  } 

  uprocess_table.processes[uprocess_table.size++] = proc;

  uint64_t stage[SPAWN_MAX_PAGES];
  uint64_t staged = 0;
  uint64_t as = 0;
  uint64_t stack_page = 0;

  for (uint64_t i = 0; i < pages; i++) {
    stage[i] = sys_call(SYS_ALLOC_PAGE, 0, 0, 0, 0);

    if (!stage[i]) {
      goto cleanup;
    }

    staged = i + 1;
  }

  if (!load_program(path, stage, size)) {
    goto cleanup;
  }

  as = sys_call(SYS_AS_CREATE, 0, 0, 0, 0);

  if (!as) {
    goto cleanup;
  }

  stack_page = sys_call(SYS_ALLOC_PAGE, 0, 0, 0, 0);

  if (!stack_page) {
    goto cleanup;
  }

  if (!sys_call(SYS_AS_MAP, as, USER_STACK_PAGE, stack_page, 0x07)) {
    goto cleanup;
  }

  for (uint64_t i = 0; i < pages; i++) {
    if (!sys_call(SYS_AS_MAP, as, USER_CODE_VIRT + i * 4096, stage[i], 0x07)) {
      goto cleanup;
    }
  }

  staged = 0;
  stack_page = 0;

  uint64_t thread_handle = sys_call(SYS_CREATE_THREAD, USER_CODE_VIRT, as, (uint64_t)USER_STACK_PAGE + 4096, 0);

  if (!thread_handle) {
    goto cleanup;
  }

  proc->pid = next_pid++;
  proc->as_handle = as;
  proc->main_thread = thread_handle;

  return proc;

cleanup:
  for (uint64_t i = 0; i < staged; i++) {
    sys_call(SYS_FREE_PAGE, stage[i], 0, 0, 0);
  }

  if (stack_page) {
    sys_call(SYS_FREE_PAGE, stack_page, 0, 0, 0);
  }

  if (as) {
    sys_call(SYS_AS_FREE, as, 0, 0, 0);
  }
  
  uprocess_table.processes[uprocess_table.size - 1] = NULL;
  free(proc);

  return NULL;
}

void _start(void) {
  malloc_init();

  uprocess_table.size = 0;
  uprocess_table.cap = 4;
  uprocess_table.processes = malloc(sizeof(uprocess_t) * 4);

  if (!uprocess_table.processes) {
    char buf[] = "Failed to initialize process table.";
    sys_call(SYS_WRITE, 1, (uint64_t)buf, sizeof(buf), 0);
    for (;;) { __asm__ volatile ("pause"); };
  }

  if (!spawn_process("/bin/sh")) {
    char buf[] = "Failed to startup /bin/sh.";
    sys_call(SYS_WRITE, 1, (uint64_t)buf, sizeof(buf), 0);
  }

  for (;;) { __asm__ volatile ("pause"); };

  sys_exit(0);
}
