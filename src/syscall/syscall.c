#include "../../include/lumen/syscall/syscall.h"
#include "../../include/lumen/fs/vfs.h"
#include "../../include/lumen/exec/exec.h"
#include "../../include/lumen/drivers/video/console/console.h"
#include "../../include/lumen/drivers/kbd/kbd.h"
#include "../../include/lumen/thread/thread.h"
#include "../../include/lumen/scheduler/scheduler.h"
#include "../../include/lumen/arch/x86_64/gdt.h"
#include "../../include/lumen/arch/x86_64/context.h"
#include "../../include/lumen/mm/heap.h"
#include "../../include/lumen/mm/vmm.h"
#include "../../include/lumen/lib/utils.h"
#include "../../include/lumen/ipc/ipc.h"
#include "../../include/lumen/arch/x86_64/asm/cpu.h"
#include "../../include/lumen/arch/x86_64/asm/io.h"
#include "../../include/lumen/mm/pmm.h"
#include "../../include/lumen/as/as.h"
#include "../../include/lumen/mm/dma.h"
#include "../../include/lumen/arch/x86_64/apic/apic.h"
#include "../../include/lumen/kernel/rust.h"

extern void lumen_process_syscall(syscall_frame_t *frame);
extern void switch_context(context_t *context);

static bool user_range_ok(uint64_t addr, uint64_t len) {
  if (len > SYS_IPC_MAX_MESSAGE) return false;
  if (addr < USER_AREA_BASE) return false;
  if (len == 0) return addr < USER_AREA_END;
  if (addr + len < addr) return false;
  if (addr + len > USER_AREA_END) return false;

  return true;
}

static bool user_page_ok(uint64_t virt) {
  return user_range_ok(virt, 0) && !is_shared_user_page(virt);
}

static bool user_pages_ok(uint64_t addr, uint64_t len, uint64_t flags) {
  if (!user_range_ok(addr, len)) return false;
  for (uint64_t page = addr & ~0xFFFULL; len && page < addr + len; page += 4096) {
    uint64_t *pte = kget_pte_addr(page);
    if (!pte || (*pte & flags) != flags) return false;
  }
  return true;
}

static int copy_user_path(uint64_t uaddr, char *out) {
  if (uaddr < USER_AREA_BASE || uaddr >= USER_AREA_END) return -1;

  for (size_t i = 0; i < SYSCALL_MAX_PATH; i++) {
    if (!user_pages_ok(uaddr + i, 1, 0x05)) return -1;

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
  if (!user_pages_ok(uaddr, len, 0x05)) return -1;

  memcpy(kbuf, (void *)uaddr, (size_t)len);

  return 0;
}

static int copy_to_user(uint64_t uaddr, const void *kbuf, uint64_t len) {
  if (len == 0) return 0;
  if (!kbuf) return -1;
  if (!user_pages_ok(uaddr, len, 0x07)) return -1;

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

  if (next->kernel_stack) {
    tss.rsp0 = ((uint64_t)next->kernel_stack + 4096 * KERNEL_STACK_SIZE_IN_PAGES);
  }

  switch_context(next->context);

  for (;;) __asm__ volatile ("hlt");
}

extern char syscall_return[];

static void sys_yield(syscall_frame_t *f) {
  thread_t *next = scheduler_next();

  if (!next || next == current_thread || next->state != THREAD_READY) {
    // Nothing else to run
    f->rax = 0;
    return;
  }

  f->rax = 0;

  context_t *ctx = current_thread->context;

  ctx->rsp = (uint64_t)f;
  ctx->rip = (uint64_t)syscall_return;
  ctx->rflags = read_flags();
  ctx->cs = GDT_KCODE_LM_SEGMENT;
  ctx->ss = GDT_KDATA_SEGMENT;

  current_thread = next;

  if (next->kernel_stack) {
    tss.rsp0 = ((uint64_t)next->kernel_stack + 4096 * KERNEL_STACK_SIZE_IN_PAGES);
  }

  switch_context(next->context);

  __builtin_unreachable();
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

static void exec_reap(thread_t *child) {
  for (uint64_t i = 0; i < threads_length; i++) {
    if (threads[i] == child) {
      for (uint64_t j = i; j + 1 < threads_length; j++) 
        threads[j] = threads[j + 1];
      threads_length--;
      break;
    }
  }
  
  kvmm_free_user_pml4(child->context->cr3);
  free_thread(child);
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

  uint8_t signature[4];
  if (vfs_read(path, signature, sizeof(signature), 0) == sizeof(signature) &&
      signature[0] == 0x7f && signature[1] == 'E' &&
      signature[2] == 'L' && signature[3] == 'F') {
    uint8_t args[512];
    if (f->rdx > sizeof(args) || copy_from_user(f->rsi, args, f->rdx) != 0) {
      f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
      return;
    }
    f->rax = (uint64_t)lumen_exec_elf(path, kstrlen(path) + 1, args, f->rdx, (uint64_t)size);
    return;
  }

  if (size == 0 || (uint64_t)size > PROG_MAX) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  uint64_t pages = ((uint64_t)size + 4095) / 4096;
  
  cli();

  uint64_t cr3 = kvmm_create_user_pml4();

  if (!cr3) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    sti();
    return;
  }

  if (!lumen_map_exec_stack(cr3, PROG_STACK_PAGE)) {
    kvmm_free_user_pml4(cr3);
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    sti();
    return;
  }

  thread_t *child = spawn_thread((void (*)(void))PROG_BASE, cr3);
  
  if (!child) {
    kvmm_free_user_pml4(cr3);
    sti();
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  for (uint64_t i = 0; i < PROG_PAGES; i++) {
    kvmunmap_and_free_at(child->context->cr3, PROG_BASE + i * 4096);
  }

  for (uint64_t i = 0; i < pages; i++) {
    uint64_t phys = (uint64_t)kpalloc();

    if (!phys) {
      for (uint64_t j = 0; j < i; j++) {
        kvmunmap_and_free_at(child->context->cr3, PROG_BASE + j * 4096);
      }

      exec_reap(child);
      sti();

      f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
      return;
    }

    memset((void*)phys, 0, 0x1000);
    kvmmap_at(child->context->cr3, PROG_BASE + i * 4096, phys, 0x07);
  }
  
  uint64_t old = read_cr3();
  write_cr3(child->context->cr3);
  
  int64_t read = vfs_read(path, (void*)PROG_BASE, (uint64_t)size, 0);
  
  write_cr3(old);

  if (read < 0 || read != size) {
    for (uint64_t i = 0; i < pages; i++) {
      kvmunmap_and_free_at(child->context->cr3, PROG_BASE + i * 4096);
    }

    exec_reap(child);
    sti();

    f->rax = (uint64_t)read;
    return;
  }

  child->context->cs = 0x18 | 3;
  child->context->ss = 0x20 | 3;
  child->context->rsp = PROG_STACK_TOP;
  
  sti();

  while (child->state != THREAD_DEAD) {
    __asm__ volatile ("sti; hlt; cli" ::: "memory");
  }

  uint64_t code = child->exit_code;

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
  uint64_t target = f->rdi;
  uint64_t ubuf = f->rsi;
  uint64_t size = f->rdx;

  if (size == 0 || size > SYS_IPC_MAX_MESSAGE) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
    return;
  }

  // Verify every page before copying in the current address space.
  if (!user_range_ok(ubuf, size)) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
    return;
  }
  for (uint64_t page = ubuf & ~0xFFFULL; page < ubuf + size; page += 4096) {
    uint64_t *pte = kget_pte_addr_at(current_thread->context->cr3, page);
    if (!pte || (*pte & 0x005) != 0x005) {
      f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
      return;
    }
  }

  ipc_msg_t msg = {
    .target = target,
    .sender = current_thread->tid,
    .msg = kipc_alloc_payload(size),
    .size = size
  };
  if (!msg.msg) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_NOSPACE;
    return;
  }

  memcpy(msg.msg, (const void*)ubuf, size);
  f->rax = kipc_send(msg);
  if (!f->rax) kipc_free_message(&msg);
}

void sys_ipc_poll(syscall_frame_t *f) {
  uint64_t uaddr = f->rdi;
  uint64_t capacity = f->rsi;
  uint64_t sender_out = f->rdx;

  ipc_msg_t msg;

  if (!kipc_poll(&msg)) {
    f->rax = false;
    return;
  }

  if (msg.size > capacity) {
    kipc_free_message(&msg);
    f->rax = 0;
    return;
  }

  if (copy_to_user_at(current_thread->context->cr3, uaddr, msg.msg, msg.size) != 0) {
    kipc_free_message(&msg);
    f->rax = 0;
    return;
  }

  if (sender_out) {
    if (copy_to_user_at(current_thread->context->cr3, sender_out, &msg.sender, sizeof(uint64_t)) != 0) {
      kipc_free_message(&msg);
      f->rax = 0;
      return;
    }
  }

  kipc_free_message(&msg);
  f->rax = msg.size;
}

void sys_as_create(syscall_frame_t* f) {
  addrspace_t *as = kaddrspace_create();

  if (!as) {
    f->rax = 0;
    return;
  }

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

  f->rax = kvmmap_at(as->cr3, f->rsi, phys, f->r10 & 0x07);
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

  if (!t) {
    f->rax = 0;
    return;
  }

  t->context->cs = 0x18 | 3;
  t->context->ss = 0x20 | 3;
  t->context->rsp = f->rdx;

  f->rax = t->tid;
}

void sys_alloc_page(syscall_frame_t* f) {
  uint64_t virt = user_find_free_page(current_thread->context->cr3);

  if (!virt) {
    f->rax = 0;
    return;
  }

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

void sys_dma_alloc_page(syscall_frame_t* f) {
  void* dma_page = kdma_palloc();

  if (!dma_page) {
    f->rax = 0;
    return;
  }

  uint64_t phys = (uint64_t)dma_page;

  uint64_t offset = phys - DMA_PAGES;
  uint64_t n = offset / 4096;
  uint64_t virt = DMA_VIRT_BASE + n * 4096;

  if (!kvmmap_at(current_thread->context->cr3, virt, phys, 0x07)) {
    kdma_pfree(dma_page);
    f->rax = 0;
    return;
  }

  if (copy_to_user_at(current_thread->context->cr3, f->rdi, &phys, sizeof(phys)) != 0) {
    kvmunmap_at(current_thread->context->cr3, virt);
    kdma_pfree(dma_page);
    f->rax = 0;
    return;
  }

  f->rax = virt;
}

void sys_dma_free_page(syscall_frame_t* f) {
  uint64_t virt = f->rdi;
  uint64_t offset = virt - DMA_VIRT_BASE;
  f->rax = 0;

  if (offset >= DMA_PAGE_AMOUNT * 4096 || (offset & 0xFFF)) return;

  uint64_t cr3 = current_thread->context->cr3;
  uint64_t phys = DMA_PAGES + offset;
  uint64_t *pte = kget_pte_addr_at(cr3, virt);
  if (!pte || (*pte & 0x005) != 0x005 || (*pte & ~0xFFFULL) != phys) return;

  kvmunmap_at(cr3, virt);
  kdma_pfree((void*)phys);
  f->rax = 1;
}

void sys_mmap(syscall_frame_t* f) {
  uint64_t virt = f->rdi;
  uint64_t phys = f->rsi;
  
  if ((virt | phys) & 0xFFF || !kis_normal_ram(phys)) { 
    f->rax = 0;
    return;
  }

  if (!user_page_ok(virt)) {
    f->rax = 0;
    return;
  }

  f->rax = kvmmap_at(current_thread->context->cr3, virt, phys, f->rdx & 0x07);
}

void sys_map_mmio(syscall_frame_t* f) {
  uint64_t virt = f->rdi;
  uint64_t phys = f->rsi;
  uint64_t pages = f->rdx;

  if (!current_thread || !current_thread->context) {
    f->rax = 0;
    return;
  }

  if (!pages || pages > 256 || ((virt | phys) & 0xFFF)) {
    f->rax = 0;
    return;
  }

  // Device mappings live above the kernel identity map: below 1 GiB every PTE
  // is present, and most of those page tables are shared with the kernel.
  if (virt < USER_MMIO_MIN || virt + pages * 0x1000 < virt || virt + pages * 0x1000 > USER_MMIO_MAX || phys + pages * 0x1000 < phys) {
    f->rax = 0;
    return;
  }

  for (uint64_t i = 0; i < pages; i++) {
    uint64_t *pte = kget_pte_addr_at(current_thread->context->cr3, virt + i * 0x1000);

    if (kis_normal_ram(phys + i * 0x1000) || (pte && (*pte & 1))) {
      f->rax = 0;
      return;
    }
  }

  for (uint64_t i = 0; i < pages; i++) {
    if (!kvmmap_at(current_thread->context->cr3, virt + i * 0x1000, phys + i * 0x1000, 0x1F)) {
      while (i > 0) {
        i--;
        kvmunmap_at(current_thread->context->cr3, virt + i * 0x1000);
      }
      f->rax = 0;
      return;
    }
  }

  f->rax = virt;
}

void sys_free_thread(syscall_frame_t* f) {
  if (!f->rdi || !current_thread || f->rdi == current_thread->tid) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
    return;
  }
  
  thread_t *t = NULL;

  for (uint64_t i = 0; i < threads_length; i++) {
    if (threads[i]->tid == f->rdi) {
      t = threads[i];
      break;
    }
  }

  if (!t || t->elf_process || t->context->cs == GDT_KCODE_LM_SEGMENT) {
    f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
    return;
  }

  for (uint64_t i = 0; i < threads_length; i++) {
    if (threads[i] == t) {
      for (uint64_t j = i; j + 1 < threads_length; j++) 
        threads[j] = threads[j + 1];
      threads_length--;
      break;
    }
  }

  free_thread(t);
  f->rax = 0;
}

void sys_free_page(syscall_frame_t* f) {
  if (!f->rdi || !user_page_ok(f->rdi)) {
    f->rax = 0;
    return;
  }
  
  f->rax = kvmunmap_and_free_at(current_thread->context->cr3, f->rdi);
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
  if (!f->rdi || !user_page_ok(f->rdi)) {
    f->rax = 0;
    return;
  }
  
  kvmunmap_at(current_thread->context->cr3, f->rdi);
  f->rax = 1;
}

void sys_unmmap_and_free(syscall_frame_t *f) {
  if (!f->rdi || !user_page_ok(f->rdi)) {
    f->rax = 0;
    return;
  }

  f->rax = kvmunmap_and_free_at(current_thread->context->cr3, f->rdi);
}

void sys_pci_conf_read32(syscall_frame_t *f) {
  uint64_t bus = f->rdi;
  uint64_t dev = f->rsi;
  uint64_t func = f->rdx;
  uint64_t offset = f->r10;

  if (bus > 0xFF || dev > 0x1F || func > 0x07 || offset > 0xFC) {
    f->rax = 0; // not-existent
    return;
  }

  uint32_t address = 0x80000000u | ((uint32_t)bus << 16) | ((uint32_t)dev << 11) | ((uint32_t)func << 8) | (offset & 0xFC);
  outl(0xCF8, address);
  
  f->rax = inl(0xCFC);
}

// Lumen currently runs a single NVMe service. Only alter the command word,
// leaving PCI status (whose bits may be write-one-to-clear) untouched.
static void sys_nvme_claim(syscall_frame_t *f) {
  static uint64_t owner;
  static uint32_t config;
  static uint16_t original_command;

  f->rax = 0;
  if (!current_thread) return;
  if (f->rsi == 1) {
    if (owner != current_thread->tid) return;
    outl(0xCF8, config + 4);
    outw(0xCFC, original_command);
    owner = 0;
    f->rax = 1;
    return;
  }
  if (f->rsi != 0 || owner || f->rdi > 0xFFFF) return;

  uint32_t address = 0x80000000U | ((uint32_t)f->rdi << 8);
  outl(0xCF8, address + 8);
  if (inl(0xCFC) >> 8 != 0x010802) return;

  config = address;
  outl(0xCF8, config + 4);
  original_command = inw(0xCFC);
  // Enable memory decoding and DMA. Disable INTx for this polled driver.
  outw(0xCFC, original_command | 0x406);
  owner = current_thread->tid;
  f->rax = 1;
}

void syscall_handler(syscall_frame_t *f) {
  if (!f) return;
  if (current_thread && current_thread->elf_process) {
    lumen_process_syscall(f);
    return;
  }

  switch (f->rax) {
    case SYS_CLOCK_MS:
      f->rax = lapic_ticks_ms;
      break;
    case SYS_NVME_CLAIM:
      sys_nvme_claim(f);
      break;
    case SYS_GET_SELF_TID:
      f->rax = current_thread ? current_thread->tid : 0;
      break;
  
    case SYS_EXIT:
      sys_exit(f);
      break;

    case SYS_WRITE:
      sys_write(f);
      break;

    case SYS_YIELD:
      sys_yield(f);
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

    case SYS_DMA_ALLOC_PAGE:
      sys_dma_alloc_page(f);
      break;

    case SYS_DMA_FREE_PAGE:
      sys_dma_free_page(f);
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

    case SYS_MAP_MMIO:
      sys_map_mmio(f);
      break;

    case SYS_PCI_CONF_READ32:
      sys_pci_conf_read32(f);
      break;

    default:
      f->rax = (uint64_t)(int64_t)VFS_ERR_INVAL;
      break;
  }
}

bool syscall_init(void) {
  return true;
}
