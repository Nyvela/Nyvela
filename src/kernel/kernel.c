#include "../../include/nyvela/drivers/video/console/console.h"
#include "../../include/nyvela/lib/utils.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/mm/vmm.h"
#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/arch/x86_64/idt.h"
#include "../../include/nyvela/arch/x86_64/apic/apic.h"
#include "../../include/nyvela/arch/x86_64/asm/cpu.h"
#include "../../include/nyvela/thread/thread.h"
#include "../../include/nyvela/arch/x86_64/gdt.h"
#include "../../include/nyvela/arch/x86_64/pic/pic.h"
#include "../../include/nyvela/arch/x86_64/pit/pit.h"
#include "../../include/nyvela/fs/vfs.h"
#include "../../include/nyvela/exec/exec.h"
#include "../../include/nyvela/syscall/syscall.h"
#include "../../include/nyvela/drivers/kbd/kbd.h"
#include "../../include/nyvela/ktests/ktests.h"
#include "../../include/nyvela/drivers/video/vga/vga.h"
#include "../../include/nyvela/process/process.h"

extern uint8_t shell_blob_start[];
extern uint8_t shell_blob_end[];
extern uint8_t hello_blob_start[];
extern uint8_t hello_blob_end[];

void krnl() {
  for (;;) {
    hlt();
  }
}

static void kload_user_bins(void) {
  if (vfs_create("/bin", true) != VFS_OK) {
    kprintferr("Failed to create /bin.", 0x0F);
    hang();
  }

  uint64_t shell_size = (uint64_t)(shell_blob_end - shell_blob_start);

  if (shell_size == 0 || shell_size > VFS_MAX_FILE_SIZE) {
    kprintferr("Bad shell blob size.", 0x0F);
    hang();
  }

  if (vfs_create("/bin/sh", false) != VFS_OK) {
    kprintferr("Failed to create /bin/sh.", 0x0F);
    hang();
  }

  if (vfs_write("/bin/sh", shell_blob_start, shell_size, 0) != (int64_t)shell_size) {
    kprintferr("Failed to write /bin/sh.", 0x0F);
    hang();
  }

  uint64_t hello_size = (uint64_t)(hello_blob_end - hello_blob_start);

  if (hello_size == 0 || hello_size > VFS_MAX_FILE_SIZE) {
    kprintferr("Bad hello blob size.", 0x0F);
    hang();
  }

  if (vfs_create("/bin/hello", false) != VFS_OK) {
    kprintferr("Failed to create /bin/hello.", 0x0F);
    hang();
  }

  if (vfs_write("/bin/hello", hello_blob_start, hello_size, 0) != (int64_t)hello_size) {
    kprintferr("Failed to write /bin/hello.", 0x0F);
    hang();
  }

  static const char readme[] =
    "Welcome to Nyvela!\n"
    "Try: help, ls, ls /bin, cat /readme.txt, run hello\n";

  if (vfs_create("/readme.txt", false) != VFS_OK) {
    kprintferr("Failed to create /readme.txt.", 0x0F);
    hang();
  }

  uint64_t rlen = sizeof(readme) - 1;

  if (vfs_write("/readme.txt", readme, rlen, 0) != (int64_t)rlen) {
    kprintferr("Failed to write /readme.txt.", 0x0F);
    hang();
  }
}

void kuserspace_init() {
  process_t *idle = spawn_process(krnl);

  if (!idle) {
    kprintferr("Failed to spawn idle process.", 0x0F);
    hang();
  }

  process_t *user = spawn_process((void (*)(void))USER_CODE_VIRT);

  if (!user) {
    kprintferr("Failed to spawn shell process.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }

  for (uint64_t i = 0; i < SHELL_PAGES; i++) {
    uint64_t phys = (uint64_t)kpalloc();

    if (!phys) {
      kprintferr("Failed to allocate memory for shell.", 0x0F);
      hang();
    }

    if (!kvmmap_at(user->cr3, USER_CODE_VIRT + i * 4096, phys, 0x07)) {
      kprintferr("Failed to map shell.", 0x0F);
      hang();
    }
  }
  
  uint64_t old_cr3 = read_cr3();
  write_cr3(user->cr3);

  if (exec_load("/bin/sh", USER_CODE_VIRT, SHELL_MAX) < 0) {
    kprintferr("Failed to load /bin/sh.", 0x0F);
    hang();
  }

  write_cr3(old_cr3);

  uint64_t stack_phys = (uint64_t)kpalloc();

  if (!stack_phys) {
    kprintferr("Failed to allocate stack for shell.", 0x0F);
    hang();
  }

  if (!kvmmap_at(user->cr3, USER_STACK_PAGE, stack_phys, 0x07)) {
    kprintferr("Failed to map shell stack.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }

  memset((void *)stack_phys, 0, 4096);

  for (uint64_t i = 0; i < PROG_PAGES; i++) {
    uint64_t phys = (uint64_t)kpalloc();
  
    if (!phys) {
      kprintferr("Failed to allocate program slot.", 0x0F);
      hang();
    }

    if (!kvmmap_at(user->cr3, PROG_BASE + i * 4096, phys, 0x07)) {
      kprintferr("Failed to map program slot.", 0x0F);
      hang();
    }

    memset((void *)phys, 0, 4096);
  }
  
  uint64_t prog_stack = (uint64_t)kpalloc();

  if (!prog_stack) {
    kprintferr("Failed to allocate program stack.", 0x0F);
    hang();
  }

  if (!kvmmap_at(user->cr3, PROG_STACK_PAGE, prog_stack, 0x07)) {
    kprintferr("Failed to map program stack.", 0x0F);
    hang();
  }

  memset((void *)prog_stack, 0, 4096);

  kprintinfo("Switching to ring3 shell (/bin/sh)...", 0x0F);

  user->threads[0]->context->cs = 0x18 | 3;
  user->threads[0]->context->ss = 0x20 | 3;
  user->threads[0]->context->rsp = USER_STACK_TOP;

  current_thread = user->threads[0];
  tss.rsp0 = ((uint64_t)user->threads[0]->kernel_stack + 4096 * KERNEL_STACK_SIZE_IN_PAGES);
  switch_context(user->threads[0]->context);
}

__attribute__((section(".text.entry")))
void kmain() {
  kprintsucc("Nyvela kernel started.", 0x0F);

  klog_ram_data();

  kprintinfo("Initializing PMM...", 0x0F);
  
  if (!kpmm_init()) {
    kprintferr("PMM initialization failed.", 0x0F);
    hang();
  }
  
  kprintsucc("PMM ready.", 0x0F);

  ktest_palloc();

  kprintinfo("Initializing VMM...", 0x0F);
  
  if (!kvmm_init()) {
    kprintferr("VMM initialization failed.", 0x0F);
    hang();
  }
  
  kprintsucc("VMM ready.", 0x0F);

  ktest_palloc();
  ktest_vmmap();

  kprintinfo("Initializing kmalloc...", 0x0F);
  
  if (!kmalloc_init()) {
    kprintferr("kmalloc initialization failed.", 0x0F);
    hang();
  }
  
  kprintsucc("kmalloc ready.", 0x0F);

  ktest_kmalloc();
  ktest_krealloc();

  if (!ktss_init()) {
    kprintferr("Failed to initialize TSS.", 0x0F);
    hang();
  }

  kprintsucc("Initialized TSS.", 0x0F);

  kprintinfo("Initializing IDT (incl. syscall 0x80)...", 0x0F);
  
  if (!kidt_init()) {
    kprintferr("IDT initialization failed.", 0x0F);
    hang();
  }
  
  kprintsucc("IDT ready.", 0x0F);  

  kpic_init();

  kprintsucc("Initiliazed PIC.", 0x0F);

  kpit_set_freq(1000);

  kprintsucc("Set PIT frequency to 1000 Hz.", 0x0F);
  
  if (!kenable_lapic()) {
    kprintferr("Failed to enable LAPIC.", 0x0F);
    hang();
  }

  kprintsucc("LAPIC enabled.", 0x0F);
  
  ksetup_lapic_timer();
  
  kprintsucc("LAPIC timer setup complete.", 0x0F);

  kpic_disable();

  kbd_init();

  kprintsucc("Keyboard ready (IRQ1).", 0x0F);
    
  kprintinfo("Initializing VFS (ramfs at /)...", 0x0F);

  if (!vfs_init()) {
    kprintferr("VFS initialization failed.", 0x0F);
    hang();
  }

  if (!syscall_init()) {
    kprintferr("Syscall initialization failed.", 0x0F);
    hang();
  }

  kprintsucc("VFS ready.", 0x0F);

  kload_user_bins();

  kprintsucc("User binaries ready (/bin/hello).", 0x0F);

  ktest_vfs();
  ktest_syscall();

  kuserspace_init();

  for (;;) {
    hlt();
  }
}
