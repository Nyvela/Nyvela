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

extern uint8_t shell_blob_end[], shell_blob_start[];
extern uint8_t hello_blob_end[], hello_blob_start[];
extern uint8_t init_blob_end[], init_blob_start[];
extern uint8_t nvmed_blob_end[], nvmed_blob_start[];

void krnl() {
  for (;;) {
    hlt();
  }
}

static void kload_user_bin(char *path, uint8_t* blob_start, uint8_t* blob_end) {
  uint64_t bin_size = (uint64_t)(blob_end - blob_start);

  if (bin_size == 0 || bin_size > VFS_MAX_FILE_SIZE) {
    kprintferr("Bad Blob Size.", 0x0F);
    hang();
  }

  if (vfs_create(path, false) != VFS_OK) {
    kprintferr("Failed to create user binary.", 0x0F);
    hang();
  }

  if (vfs_write(path, blob_start, bin_size, 0) != (int64_t)bin_size) {
    kprintferr("Failed to write user binary.", 0x0F);
    hang();
  }
}

static void kload_user_bins(void) {
  if (vfs_create("/bin", true) != VFS_OK) {
    kprintferr("Failed to create /bin.", 0x0F);
    hang();
  }

  kload_user_bin("/bin/sh", shell_blob_start, shell_blob_end);
  kload_user_bin("/bin/nvmed", nvmed_blob_start, nvmed_blob_end);
  kload_user_bin("/bin/hello", hello_blob_start, hello_blob_end);
  kload_user_bin("/bin/init", init_blob_start, init_blob_end);

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

  process_t *init = spawn_process((void (*)(void))USER_CODE_VIRT);

  if (!init) {
    kprintferr("Failed to spawn init process.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }

  uint64_t code_page_count = (init_blob_end - init_blob_start + 4095) / 4096;

  for (uint64_t i = 0; i < code_page_count; i++) {
    uint64_t phys = (uint64_t)kpalloc();

    if (!phys) {
      kprintferr("Failed to allocate memory for init process.", 0x0F);
      hang();
    }

    if (!kvmmap_at(init->cr3, USER_CODE_VIRT + i * 4096, phys, 0x07)) {
      kprintferr("Failed to map shell.", 0x0F);
      hang();
    }
  }
  
  uint64_t old_cr3 = read_cr3();
  write_cr3(init->cr3);

  if (exec_load("/bin/init", USER_CODE_VIRT, code_page_count * 4096) < 0) {
    kprintferr("Failed to load /bin/init.", 0x0F);
    hang();
  }

  write_cr3(old_cr3);

  uint64_t stack_phys = (uint64_t)kpalloc();

  if (!stack_phys) {
    kprintferr("Failed to allocate stack for init process.", 0x0F);
    hang();
  }

  if (!kvmmap_at(init->cr3, USER_STACK_PAGE, stack_phys, 0x07)) {
    kprintferr("Failed to map init process stack.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }

  memset((void *)stack_phys, 0, 4096);

  for (uint64_t i = 0; i < PROG_PAGES; i++) {
    uint64_t phys = (uint64_t)kpalloc();
  
    if (!phys) {
      kprintferr("Failed to allocate program slot.", 0x0F);
      hang();
    }

    if (!kvmmap_at(init->cr3, PROG_BASE + i * 4096, phys, 0x07)) {
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

  if (!kvmmap_at(init->cr3, PROG_STACK_PAGE, prog_stack, 0x07)) {
    kprintferr("Failed to map program stack.", 0x0F);
    hang();
  }

  memset((void *)prog_stack, 0, 4096);

  kprintinfo("Switching to ring3 shell (/bin/init)...", 0x0F);

  init->threads[0]->context->cs = 0x18 | 3;
  init->threads[0]->context->ss = 0x20 | 3;
  init->threads[0]->context->rsp = USER_STACK_TOP;

  current_thread = init->threads[0];
  tss.rsp0 = ((uint64_t)init->threads[0]->kernel_stack + 4096 * KERNEL_STACK_SIZE_IN_PAGES);
  switch_context(init->threads[0]->context);
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
