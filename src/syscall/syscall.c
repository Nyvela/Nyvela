#include "../../include/nyvela/syscall/syscall.h"
#include "../../include/nyvela/fs/vfs.h"
#include "../../include/nyvela/exec/exec.h"
#include "../../include/nyvela/drivers/video/console/console.h"
#include "../../include/nyvela/drivers/kbd/kbd.h"
#include "../../include/nyvela/thread/thread.h"
#include "../../include/nyvela/scheduler/scheduler.h"
#include "../../include/nyvela/arch/x86_64/gdt.h"
#include "../../include/nyvela/arch/x86_64/context.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/mm/vmm.h"
#include "../../include/nyvela/lib/utils.h"

extern void switch_context(context_t *context);

static bool user_range_ok(uint64_t addr, uint64_t len) {
  if (len > SYSCALL_MAX_BUF + SYSCALL_MAX_PATH) return false;
  if (addr < USER_AREA_BASE) return false;
  if (len == 0) return addr < USER_AREA_END;
  if (addr + len < addr) return false; // overflow
  if (addr + len > USER_AREA_END) return false;

  return true;
}

// Copy NUL-terminated path from user window into kernel out[257].
// Returns 0 on success, -1 on bad pointer / unterminated / too long.
static int copy_user_path(uint64_t uaddr, char *out) {
  if (uaddr < USER_AREA_BASE || uaddr >= USER_AREA_END) return -1;

  for (size_t i = 0; i < SYSCALL_MAX_PATH; i++) {
    if (uaddr + i >= USER_AREA_END) return -1;

    char c = *(volatile char *)(uaddr + i);
    out[i] = c;

    if (c == '\0') return 0;
  }

  out[SYSCALL_MAX_PATH] = '\0';

  return -1; // unterminated
}

static int copy_from_user(uint64_t uaddr, void *kbuf, uint64_t len) {
  if (len == 0) return 0;
  if (!kbuf) return -1;
  if (!user_range_ok(uaddr, len)) return -1;

  memcpy(kbuf, (void *)uaddr, (size_t)len);

  return 0;
}

static int copy_to_user(uint64_t uaddr, const void *kbuf, uint64_t len) {
  if (len == 0) return 0;
  if (!kbuf) return -1;
  if (!user_range_ok(uaddr, len)) return -1;

  memcpy((void *)uaddr, kbuf, (size_t)len);

  return 0;
}

static void sys_exit(syscall_frame_t *f) {
  if (!current_thread) {
    __asm__ volatile ("cli; hlt");
    for (;;);
  }

  current_thread->exit_code = (int64_t)f->rdi;
  current_thread->state = THREAD_DEAD;

  thread_t *next = scheduler_next();

  // scheduler_next returns current when nothing READY; detect dead-end.
  if (!next || next == current_thread || next->state != THREAD_READY) {
    kprintferr("sys_exit: no runnable thread.", 0x0F);
    __asm__ volatile ("cli; hlt");
    for (;;);
  }

  current_thread = next;

  if (next->kernel_stack) {
    tss_set_rsp0((uint64_t)next->kernel_stack + 4096);
  }

  switch_context(next->context);

  for (;;) __asm__ volatile ("hlt");
}

static void sys_write(syscall_frame_t *f) {
  uint64_t fd = f->rdi;
  uint64_t ubuf = f->rsi;
  uint64_t len = f->rdx;

  if (fd != 1 && fd != 2) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
    return;
  }

  if (len > SYSCALL_MAX_BUF) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  if (len == 0) {
    f->rax = 0;
    return;
  }

  char *kbuf = kmalloc(len);

  if (!kbuf) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  if (copy_from_user(ubuf, kbuf, len) != 0) {
    kfree(kbuf);
    f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
    return;
  }

  kwrite(kbuf, len, 0x0F);
  kfree(kbuf);
  f->rax = len;
}

// Blocking stdin read (fd 0). Returns as soon as >=1 byte is available,
// up to len. Waits with sti/hlt so the timer keeps preempting us while
// blocked (per-thread kernel stacks make this safe).
static void sys_read(syscall_frame_t *f) {
  uint64_t fd = f->rdi;
  uint64_t ubuf = f->rsi;
  uint64_t len = f->rdx;

  if (fd != 0) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
    return;
  }

  if (len == 0) {
    f->rax = 0;
    return;
  }

  if (len > SYSCALL_MAX_BUF) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  char *kbuf = kmalloc(len);

  if (!kbuf) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  uint64_t got = 0;

  for (;;) {
    while (got < len) {
      int c = kbd_getc();

      if (c < 0) break;

      kbuf[got++] = (char)c;
    }

    if (got) break;

    // sti;hlt is race-free: STI shadows the next insn, so a pending
    // key IRQ lands after hlt and wakes us.
    __asm__ volatile ("sti; hlt; cli" ::: "memory");
  }

  if (copy_to_user(ubuf, kbuf, got) != 0) {
    kfree(kbuf);
    f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
    return;
  }

  kfree(kbuf);
  f->rax = got;
}

// Foreground exec: load ramfs file into the single PROG_BASE slot,
// spawn it as a ring3 thread sharing our PML4, and block until it exits.
// Returns the child's exit code (or negative VFS_ERR_*).
static void sys_exec(syscall_frame_t *f) {
  char path[SYSCALL_MAX_PATH + 1];

  if (copy_user_path(f->rdi, path) != 0) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_BADPATH;
    return;
  }

  int64_t loaded = exec_load(path, PROG_BASE, PROG_MAX);

  if (loaded < 0) {
    f->rax = (uint64_t)loaded;
    return;
  }

  // cr3=0 -> inherit current (shared user PML4; kernel mappings included).
  thread_t *child = spawn_thread((void (*)(void))PROG_BASE, 0);

  if (!child) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  child->context->cs = 0x18 | 3;
  child->context->ss = 0x20 | 3;
  child->context->rsp = PROG_STACK_TOP;

  // v1: no reaping; DEAD exec threads linger (skipped by the scheduler).
  while (child->state != THREAD_DEAD) {
    __asm__ volatile ("sti; hlt; cli" ::: "memory");
  }

  f->rax = (uint64_t)child->exit_code;
}

static void sys_fs_create(syscall_frame_t *f) {
  char path[SYSCALL_MAX_PATH + 1];

  if (copy_user_path(f->rdi, path) != 0) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_BADPATH;
    return;
  }

  bool is_dir = f->rsi != 0;
  f->rax = (uint64_t)(int64_t)vfs_create(path, is_dir);
}

static void sys_fs_write(syscall_frame_t *f) {
  char path[SYSCALL_MAX_PATH + 1];
  uint64_t ubuf = f->rsi;
  uint64_t len = f->rdx;
  uint64_t offset = f->r10;

  if (copy_user_path(f->rdi, path) != 0) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_BADPATH;
    return;
  }

  if (len > SYSCALL_MAX_BUF) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  if (len == 0) {
    int64_t r = vfs_write(path, "", 0, offset);
    f->rax = (uint64_t)r;
    return;
  }

  char *kbuf = kmalloc(len);

  if (!kbuf) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  if (copy_from_user(ubuf, kbuf, len) != 0) {
    kfree(kbuf);
    f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
    return;
  }

  int64_t r = vfs_write(path, kbuf, len, offset);
  kfree(kbuf);
  f->rax = (uint64_t)r;
}

static void sys_fs_read(syscall_frame_t *f) {
  char path[SYSCALL_MAX_PATH + 1];
  uint64_t ubuf = f->rsi;
  uint64_t len = f->rdx;
  uint64_t offset = f->r10;

  if (copy_user_path(f->rdi, path) != 0) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_BADPATH;
    return;
  }

  if (len > SYSCALL_MAX_BUF) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  if (len == 0) {
    f->rax = 0;
    return;
  }

  char *kbuf = kmalloc(len);

  if (!kbuf) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  int64_t r = vfs_read(path, kbuf, len, offset);

  if (r > 0) {
    if (copy_to_user(ubuf, kbuf, (uint64_t)r) != 0) {
      kfree(kbuf);
      f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
      return;
    }
  }

  kfree(kbuf);
  f->rax = (uint64_t)r;
}

static void sys_fs_list(syscall_frame_t *f) {
  char path[SYSCALL_MAX_PATH + 1];
  uint64_t ubuf = f->rsi;
  uint64_t len = f->rdx;

  if (copy_user_path(f->rdi, path) != 0) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_BADPATH;
    return;
  }

  if (len == 0 || len > SYSCALL_MAX_BUF) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
    return;
  }

  char *kbuf = kmalloc(len);

  if (!kbuf) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  int64_t r = vfs_list(path, kbuf, len);

  if (r >= 0) {
    // vfs_list NUL-terminates; copy bytes + NUL.
    uint64_t to_copy = (uint64_t)r + 1;

    if (to_copy > len) to_copy = len;

    if (copy_to_user(ubuf, kbuf, to_copy) != 0) {
      kfree(kbuf);
      f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
      return;
    }
  }

  kfree(kbuf);
  f->rax = (uint64_t)r;
}

void syscall_handler(syscall_frame_t *f) {
  if (!f) return;

  switch (f->rax) {
    case SYS_EXIT:
      sys_exit(f);
      break; // unreachable (switches away)

    case SYS_WRITE:
      sys_write(f);
      break;

    case SYS_YIELD:
      f->rax = 0; // preemption is timer-driven; yield is a no-op stub
      break;

    case SYS_READ:
      sys_read(f);
      break;

    case SYS_FS_CREATE:
      sys_fs_create(f);
      break;

    case SYS_FS_WRITE:
      sys_fs_write(f);
      break;

    case SYS_FS_READ:
      sys_fs_read(f);
      break;

    case SYS_FS_LIST:
      sys_fs_list(f);
      break;

    case SYS_EXEC:
      sys_exec(f);
      break;

    default:
      f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
      break;
  }
}

bool syscall_init(void) {
  // IDT gate is installed by kidt_init(); nothing else needed.
  return true;
}
