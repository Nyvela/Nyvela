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
#include "../../include/nyvela/ipc/ipc.h"
#include "../../include/nyvela/arch/x86_64/asm/cpu.h"
#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/as/as.h"

extern void switch_context(context_t *context);

static bool user_range_ok(uint64_t addr, uint64_t len) {
  if (len > SYSCALL_MAX_BUF + SYSCALL_MAX_PATH) return false;
  if (addr < USER_AREA_BASE) return false;
  if (len == 0) return addr < USER_AREA_END;
  if (addr + len < addr) return false;
  if (addr + len > USER_AREA_END) return false;

  return true;
}

static int copy_user_path(uint64_t uaddr, char *out) {
  if (uaddr < USER_AREA_BASE || uaddr >= USER_AREA_END) return -1;

  for (size_t i = 0; i < SYSCALL_MAX_PATH; i++) {
    if (uaddr + i >= USER_AREA_END) return -1;

    char c = *(volatile char *)(uaddr + i);
    out[i] = c;

    if (c == '\0') return 0;
  }

  out[SYSCALL_MAX_PATH] = '\0';

  return -1;
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

static int copy_to_user_at(uint64_t cr3, uint64_t uaddr, const void *kbuf, uint64_t len) {
  if (len == 0)
    return 0;

  if (!kbuf)
    return -1;

  if (!user_range_ok(uaddr, len))
    return -1;

  const uint8_t *src = (const uint8_t *)kbuf;
  uint64_t remaining = len;
  uint64_t addr = uaddr;

  while (remaining) {
    uint64_t *pte = kget_pte_addr_at(cr3, addr);

    if (!pte)
      return -1;

    uint64_t entry = *pte;

    // Present
    if (!(entry & 0x01))
      return -1;

    // User-accessible
    if (!(entry & 0x04))
      return -1;

    // Writable
    if (!(entry & 0x02))
      return -1;

    uint64_t phys = entry & ~0xFFFULL;

    uint64_t page_offset = addr & 0xFFFULL;
    uint64_t chunk = 0x1000 - page_offset;

    if (chunk > remaining)
      chunk = remaining;

    memcpy(
      (void *)(phys + page_offset),
      src,
      (size_t)chunk
    );

    addr += chunk;
    src += chunk;
    remaining -= chunk;
  }

  return 0;
}

static uint64_t user_find_free_page(uint64_t cr3) {
  for (uint64_t virt = USER_HEAP_BASE; virt < USER_AREA_END; virt += 4096) {
    uint64_t *pte = kget_pte_addr_at(cr3, virt);
    if (!pte || !(*pte & 0x01) || !(*pte & 0x04)) return virt;
  }

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

  if (!next || next == current_thread || next->state != THREAD_READY) {
    kprintferr("sys_exit: no runnable thread.", 0x0F);
    __asm__ volatile ("cli; hlt");
    for (;;);
  }

  current_thread = next;
  current_process = next->process;

  if (next->kernel_stack) {
    tss.rsp0 = ((uint64_t)next->kernel_stack + 4096 * KERNEL_STACK_SIZE_IN_PAGES);
  }

  switch_context(next->context);

  for (;;) __asm__ volatile ("hlt");
}

static void sys_fs_size(syscall_frame_t *f) {
  char path[SYSCALL_MAX_PATH + 1];

  if (copy_user_path(f->rdi, path) != 0) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_BADPATH;
    return;
  }

  f->rax = (uint64_t)(int64_t)vfs_size(path);
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

static void exec_reap(process_t *child) {
  thread_t *t = child->threads ? child->threads[0] : NULL;

  if (t && t != current_thread) {
    for (uint64_t i = 0; i < threads_length; i++) {
      if (threads[i] == t) {
        for (uint64_t j = i; j + 1 < threads_length; j++) threads[j] = threads[j + 1];
        threads_length--;
        break;
      }
    }

    free_thread(t);
    child->threads[0] = NULL;
  }

  for (uint64_t i = 0; i < processes_length; i++) {
    if (processes[i] == child) {
      for (uint64_t j = i; j + 1 < processes_length; j++) processes[j] = processes[j + 1];
      processes_length--;
      break;
    }
  }

  free_process(child);
}

static void sys_exec(syscall_frame_t *f) {
  char path[SYSCALL_MAX_PATH + 1];

  if (copy_user_path(f->rdi, path) != 0) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_BADPATH;
    return;
  }

  int64_t size = vfs_size(path);
  if (size < 0) {
    f->rax = (uint64_t)size;
    return;
  }
  if (size == 0 || (uint64_t)size > PROG_MAX) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }
  uint64_t pages = ((uint64_t)size + 4095) / 4096;
  
  cli();

  process_t *child = spawn_process((void (*)(void))PROG_BASE);
  
  if (!child) {
    sti();
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  for (uint64_t i = 0; i < PROG_PAGES; i++) {
    kvmunmap_and_free_at(child->cr3, PROG_BASE + i * 4096);
  }

  for (uint64_t i = 0; i < pages; i++) {
    uint64_t phys = (uint64_t)kpalloc();
    if (!phys) {
      for (uint64_t j = 0; j < i; j++) {
        kvmunmap_and_free_at(child->cr3, PROG_BASE + j * 4096);
      }

      exec_reap(child);
      sti();

      f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
      return;
    }
    memset((void*)phys, 0, 0x1000);
    kvmmap_at(child->cr3, PROG_BASE + i * 4096, phys, 0x07);
  }
  
  uint64_t old = read_cr3();
  write_cr3(child->cr3);
  
  int64_t read = vfs_read(path, (void*)PROG_BASE, (uint64_t)size, 0);
  
  write_cr3(old);

  if (read < 0 || read != size) {
    for (uint64_t i = 0; i < pages; i++) {
      kvmunmap_and_free_at(child->cr3, PROG_BASE + i * 4096);
    }

    exec_reap(child);
    sti();

    f->rax = (uint64_t)read;
    return;
  }

  child->threads[0]->context->cs = 0x18 | 3;
  child->threads[0]->context->ss = 0x20 | 3;
  child->threads[0]->context->rsp = PROG_STACK_TOP;
  
  sti();

  while (child->threads[0]->state != THREAD_DEAD) {
    __asm__ volatile ("sti; hlt; cli" ::: "memory");
  }

  uint64_t code = child->threads[0]->exit_code;

  exec_reap(child);

  f->rax = code;
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

void sys_ipc_send(syscall_frame_t *f) {
  f->rax = kipc_send(f->rdi, (uint8_t*)f->rsi, f->rdx);
}

void sys_ipc_poll(syscall_frame_t *f) {
  uint64_t uaddr = f->rdi;
  uint64_t capacity = f->rsi;

  ipc_msg_t msg;

  if (!kipc_poll(&msg)) {
    f->rax = false;
    return;
  }

  if (msg.size > capacity || copy_to_user_at(current_thread->context->cr3, uaddr, msg.msg, msg.size) != 0) {
    kfree(msg.msg);
    f->rax = 0;
    return;
  }
  
  kfree(msg.msg);
  f->rax = msg.size;
}

void sys_as_create(syscall_frame_t* f) {
  addrspace_t *as = kaddrspace_create();
  f->rax = as->id;
}

void sys_as_map(syscall_frame_t* f) {
  addrspace_t *as = NULL;

  for (uint64_t i = 0; i < addrspaces_length; i++) {
    if (addrspaces[i]->id == f->rdi) {
      as = addrspaces[i];
      break;
    }
  }

  if (!as || !current_thread || !current_thread->context ||
      as->cr3 == current_thread->context->cr3) {
    f->rax = 0;
    return;
  }

  uint64_t phys = kget_phys_page_addr_at(current_thread->context->cr3, f->rdx);
  if (!phys) { f->rax = 0; return; }

  f->rax = kvmmap_at(as->cr3, f->rsi, phys, f->r10);
}

void sys_create_thread(syscall_frame_t* f) {
  addrspace_t *as = NULL;

  for (uint64_t i = 0; i < addrspaces_length; i++) {
    if (addrspaces[i]->id == f->rsi) {
      as = addrspaces[i];
      break;
    }
  }

  if (!as) {
    f->rax = 0;
    return;
  }

  thread_t *t = spawn_thread((void (*)(void))f->rdi, as->cr3);

  t->context->cs = 0x18 | 3;
  t->context->ss = 0x20 | 3;
  t->context->rsp = f->rdx;

  f->rax = t->tid;
}

void sys_alloc_page(syscall_frame_t* f) {
  uint64_t virt = user_find_free_page(current_thread->context->cr3);
  void *page = kpalloc();

  if (!page) {
    f->rax = 0;
    return;
  }

  if (!kvmmap_at(current_thread->context->cr3, virt, (uint64_t)page, 0x07)) {
    kpfree(page);
    f->rax = 0;
    return;
  }

  f->rax = virt;
}

void sys_mmap(syscall_frame_t* f) {
  f->rax = kvmmap_at(current_thread->context->cr3, f->rdi, f->rsi, f->rdx);
}

void sys_free_thread(syscall_frame_t* f) {
  if (!f->rdi) return;

  free_thread((thread_t*)f->rdi);
}

void sys_free_page(syscall_frame_t* f) {
  if (!f->rdi) return;
  
  kvmunmap_and_free_at(current_thread->context->cr3, f->rdi);
}

void sys_as_free(syscall_frame_t* f) {
  if (!f->rdi) return;

  addrspace_t *as = NULL;

  for (uint64_t i = 0; i < addrspaces_length; i++) {
    if (addrspaces[i]->id == f->rdi) {
      as = addrspaces[i];
      break;
    }
  }

  if (!as) return;
  kvmm_free_user_pml4(as->cr3);
}

void sys_unmmap(syscall_frame_t *f) {
  if (!f->rdi) return;

  kvmunmap_at(current_process->cr3, f->rdi);
}

void sys_unmmap_and_free(syscall_frame_t *f) {
  if (!f->rdi) return;

  kvmunmap_and_free_at(current_process->cr3, f->rdi);
}

void syscall_handler(syscall_frame_t *f) {
  if (!f) return;

  switch (f->rax) {
    case SYS_EXIT:
      sys_exit(f);
      break;

    case SYS_WRITE:
      sys_write(f);
      break;

    case SYS_YIELD:
      f->rax = 0;
      break;

    case SYS_READ:
      sys_read(f);
      break;

    case SYS_IPC_SEND:
      sys_ipc_send(f);
      break;

    case SYS_IPC_POLL:
      sys_ipc_poll(f);
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

    case SYS_FS_SIZE:
      sys_fs_size(f);
      break;

    case SYS_EXEC:
      sys_exec(f);
      break;

    case SYS_CLEAR:
      kclear();
      break;

    case SYS_AS_CREATE:
      sys_as_create(f);
      break;

    case SYS_AS_MAP:
      sys_as_map(f);
      break;

    case SYS_AS_FREE:
      sys_as_free(f);
      break;
    
    case SYS_CREATE_THREAD:
      sys_create_thread(f);
      break;

    case SYS_FREE_THREAD:
      sys_free_thread(f);
      break;

    case SYS_ALLOC_PAGE:
      sys_alloc_page(f);
      break;

    case SYS_FREE_PAGE:
      sys_free_page(f);
      break;
  
    case SYS_MMAP:
      sys_mmap(f);
      break;

    case SYS_UNMMAP:
      sys_unmmap(f);
      break;

    case SYS_UNMMAP_AND_FREE:
      sys_unmmap_and_free(f);
      break;

    default:
      f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
      break;
  }
}

bool syscall_init(void) {
  return true;
}
