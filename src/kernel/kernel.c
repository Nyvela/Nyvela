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

extern uint8_t shell_blob_start[];
extern uint8_t shell_blob_end[];
extern uint8_t hello_blob_start[];
extern uint8_t hello_blob_end[];

void krnl() {
  for (;;) {
    hlt();
  }
}

void klog_ram_data() {
  char buf[21];
  uint64_t count = *MMAP_COUNT;

  if (!ki64toa(count, buf, sizeof(buf))) {
    kprintfail("Cannot convert memory map count.", 0x0F);
    return;
  }

  char msg[64];
  uint64_t i = 0;

  for (; buf[i]; i++)
    msg[i] = buf[i];

  char tmp[] = " memory map entries.";
  
  for (uint64_t j = 0; tmp[j]; j++, i++)
    msg[i] = tmp[j];

  msg[i] = '\0';
  kprintinfo(msg, 0x0F);

  uint64_t total = 0;

  for (uint64_t i = 0; i < count; i++) {
    if (MMAP_ENTRIES[i].type == E820_USABLE)
      total += MMAP_ENTRIES[i].length_in_bytes;
  }

  if (!ki64toa(total, buf, sizeof(buf))) {
    kprintfail("Cannot convert total RAM.", 0x0F);
    return;
  }

  i = 0;

  for (; buf[i]; i++)
    msg[i] = buf[i];

  char ram[] = " bytes of usable RAM.";
  
  for (uint64_t j = 0; ram[j]; j++, i++)
    msg[i] = ram[j];

  msg[i] = '\0';
  kprintinfo(msg, 0x0F);
}

void ktest_kmalloc() {
  void* a = kmalloc(64);

  if (!a) {
    kprintferr("kmalloc failed.", 0x0F);
    return;
  }

  *(uint64_t*)a = 0x1111222233334444;

  if (*(uint64_t*)a != 0x1111222233334444) {
    kprintferr("kmalloc write failed.", 0x0F);
    return;
  }

  void* b = kmalloc(128);

  if (!b) {
    kprintferr("Second kmalloc failed.", 0x0F);
    return;
  }

  if (a == b) {
    kprintferr("kmalloc returned overlapping blocks.", 0x0F);
    return;
  }

  kfree(a);

  void* c = kmalloc(32);

  if (!c) {
    kprintferr("kmalloc failed to reuse freed block.", 0x0F);
    return;
  }

  kfree(b);
  kfree(c);

  kprintsucc("kmalloc test passed.", 0x0F);
}

void ktest_krealloc() {
  void* a = krealloc(NULL, 64);

  if (!a) {
    kprintferr("krealloc NULL failed.", 0x0F);
    return;
  }

  for (uint64_t i = 0; i < 64; i++) {
    ((uint8_t*)a)[i] = (uint8_t)(i & 0xFF);
  }

  void* b = krealloc(a, 128);

  if (!b) {
    kprintferr("krealloc grow failed.", 0x0F);
    kfree(a);
    return;
  }

  for (uint64_t i = 0; i < 64; i++) {
    if (((uint8_t*)b)[i] != (uint8_t)(i & 0xFF)) {
      kprintferr("krealloc grow corrupted data.", 0x0F);
      kfree(b);
      return;
    }
  }

  for (uint64_t i = 64; i < 128; i++) {
    ((uint8_t*)b)[i] = 0xA5;
  }

  void* c = krealloc(b, 32);

  if (!c) {
    kprintferr("krealloc shrink failed.", 0x0F);
    kfree(b);
    return;
  }

  for (uint64_t i = 0; i < 32; i++) {
    if (((uint8_t*)c)[i] != (uint8_t)(i & 0xFF)) {
      kprintferr("krealloc shrink corrupted data.", 0x0F);
      kfree(c);
      return;
    }
  }

  void* d = krealloc(c, 0);

  if (d != NULL) {
    kprintferr("krealloc zero should return NULL.", 0x0F);
    kfree(d);
    return;
  }

  void* e = kmalloc(64);

  if (!e) {
    kprintferr("krealloc setup failed.", 0x0F);
    return;
  }

  *(uint64_t*)e = 0x1111222233334444;

  void* f = krealloc(e, 8192);

  if (f != NULL) {
    kprintferr("krealloc oversize should fail.", 0x0F);
    kfree(f);
    kfree(e);
    return;
  }

  if (*(uint64_t*)e != 0x1111222233334444) {
    kprintferr("krealloc failed grow corrupted old block.", 0x0F);
    kfree(e);
    return;
  }

  kfree(e);

  kprintsucc("krealloc test passed.", 0x0F);
}

void ktest_palloc() {
  void* mem = kpalloc();

  if (!mem) {
    kprintferr("palloc failed.", 0x0F);
    return;
  }

  *(uint64_t*)mem = 0x1111222233334444;

  if (*(uint64_t*)mem != 0x1111222233334444) {
    kprintferr("palloc memory test failed.", 0x0F);
    kpfree(mem);
    return;
  }

  kpfree(mem);
}

void ktest_vmmap() {
  uint64_t phys = (uint64_t)kpalloc();
  uint64_t virt = 0x100000000;

  if (!phys) {
    kprintferr("vmmap: physical allocation failed.", 0x0F);
    return;
  }

  *(volatile uint64_t*)phys = 0x1111222233334444;

  if (!kvmmap(virt, phys, 0x03)) {
    kprintferr("vmmap: mapping failed.", 0x0F);
    kpfree((void*)phys);
    return;
  }

  if (*(volatile uint64_t*)virt != 0x1111222233334444) {
    kprintferr("vmmap: read test failed.", 0x0F);
    return;
  }

  *(volatile uint64_t*)virt = 0xAAAABBBBCCCCDDDD;

  if (*(volatile uint64_t*)phys != 0xAAAABBBBCCCCDDDD) {
    kprintferr("vmmap: write test failed.", 0x0F);
    return;
  }

  if (!kvmunmap(virt)) {
    kprintferr("vmmap: unmap failed.", 0x0F);
    return;
  }

  uint64_t cr3 = read_cr3();

  uint64_t *pml4 = (uint64_t*)(cr3 & ~0xFFFULL);

  uint64_t pml4_index = (virt >> 39) & 0x1FF;
  uint64_t pdpt_index = (virt >> 30) & 0x1FF;
  uint64_t pd_index = (virt >> 21) & 0x1FF;
  uint64_t pt_index = (virt >> 12) & 0x1FF;

  uint64_t *pdpt = (uint64_t*)(pml4[pml4_index] & ~0xFFFULL);
  uint64_t *pd = (uint64_t*)(pdpt[pdpt_index] & ~0xFFFULL);
  uint64_t *pt = (uint64_t*)(pd[pd_index] & ~0xFFFULL);

  if (pt[pt_index] & 1) {
    kprintferr("vmmap: PTE still present.", 0x0F);
    return;
  }

  kpfree((void*)phys);

  kprintsucc("vmmap test passed.", 0x0F);
}

void ktest_vfs() {
  if (vfs_create("/test", false) != VFS_OK) {
    kprintferr("vfs: create /test failed.", 0x0F);
    return;
  }

  const char msg[] = "vfs-ok";
  int64_t w = vfs_write("/test", msg, 6, 0);

  if (w != 6) {
    kprintferr("vfs: write /test failed.", 0x0F);
    return;
  }

  char buf[16];
  memset(buf, 0, sizeof(buf));

  int64_t r = vfs_read("/test", buf, sizeof(buf) - 1, 0);

  if (r != 6 || kmemcmp(buf, msg, 6) != 0) {
    kprintferr("vfs: read /test mismatch.", 0x0F);
    return;
  }

  char list[64];
  int64_t l = vfs_list("/", list, sizeof(list));

  if (l <= 0) {
    kprintferr("vfs: list / failed.", 0x0F);
    return;
  }

  bool found = false;
  size_t pos = 0;

  while (list[pos]) {
    if (kstrncmp(&list[pos], "test\n", 5) == 0) {
      found = true;
      break;
    }

    while (list[pos] && list[pos] != '\n') pos++;
    if (list[pos] == '\n') pos++;
  }

  if (!found) {
    kprintferr("vfs: /test missing in ls.", 0x0F);
    return;
  }

  kprintsucc("vfs test passed.", 0x0F);
}

void ktest_syscall() {
  uint64_t ret = 0;

  __asm__ volatile (
    "mov $2, %%rax\n"
    "int $0x80\n"
    "mov %%rax, %0\n"
    : "=r"(ret) :: "rax", "rcx", "r11", "memory"
  );

  if (ret != 0) {
    kprintferr("syscall: yield failed.", 0x0F);
    return;
  }

  __asm__ volatile (
    "mov $1, %%rax\n"
    "mov $99, %%rdi\n"
    "xor %%rsi, %%rsi\n"
    "xor %%rdx, %%rdx\n"
    "int $0x80\n"
    "mov %%rax, %0\n"
    : "=r"(ret) :: "rax", "rdi", "rsi", "rdx", "rcx", "r11", "memory"
  );

  if ((int64_t)ret != (int64_t)VFS_ERR_INVAL) {
    kprintferr("syscall: bad-fd check failed.", 0x0F);
    return;
  }

  __asm__ volatile (
    "mov $3, %%rax\n"
    "mov $99, %%rdi\n"
    "xor %%rsi, %%rsi\n"
    "xor %%rdx, %%rdx\n"
    "int $0x80\n"
    "mov %%rax, %0\n"
    : "=r"(ret) :: "rax", "rdi", "rsi", "rdx", "rcx", "r11", "memory"
  );

  if ((int64_t)ret != (int64_t)VFS_ERR_INVAL) {
    kprintferr("syscall: read bad-fd check failed.", 0x0F);
    return;
  }

  kprintsucc("syscall test passed.", 0x0F);
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
  
  if (!ktss_init()) {
    kprintferr("Failed to initialize TSS.", 0x0F);
    hang();
  }

  kprintsucc("Initialized TSS.", 0x0F);

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

  thread_t *idle = spawn_thread(krnl);

  if (!idle) {
    kprintferr("Failed to spawn idle thread.", 0x0F);
    hang();
  }

  uint64_t new_cr3 = vmm_create_user_pml4();

  if (!new_cr3) {
    kprintferr("Failed to allocate cr3 for user.", 0x0F);
    hang();
  }

  write_cr3(new_cr3);

  idle->context->cr3 = new_cr3;

  for (uint64_t i = 0; i < SHELL_PAGES; i++) {
    uint64_t phys = (uint64_t)kpalloc();

    if (!phys) {
      kprintferr("Failed to allocate memory for shell.", 0x0F);
      hang();
    }

    if (!kvmmap(USER_CODE_VIRT + i * 4096, phys, 0x07)) {
      kprintferr("Failed to map shell.", 0x0F);
      hang();
    }
  }

  if (exec_load("/bin/sh", USER_CODE_VIRT, SHELL_MAX) < 0) {
    kprintferr("Failed to load /bin/sh.", 0x0F);
    hang();
  }


  uint64_t stack_phys = (uint64_t)kpalloc();

  if (!stack_phys) {
    kprintferr("Failed to allocate stack for shell.", 0x0F);
    hang();
  }

  if (!kvmmap(USER_STACK_PAGE, stack_phys, 0x07)) {
    kprintferr("Failed to map shell stack.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }

  memset((void *)USER_STACK_PAGE, 0, 4096);

  for (uint64_t i = 0; i < PROG_PAGES; i++) {
    uint64_t phys = (uint64_t)kpalloc();
  
    if (!phys) {
      kprintferr("Failed to allocate program slot.", 0x0F);
      hang();
    }

    if (!kvmmap(PROG_BASE + i * 4096, phys, 0x07)) {
      kprintferr("Failed to map program slot.", 0x0F);
      hang();
    }

    memset((void *)(PROG_BASE + i * 4096), 0, 4096);
  }
  
  uint64_t prog_stack = (uint64_t)kpalloc();

  if (!prog_stack) {
    kprintferr("Failed to allocate program stack.", 0x0F);
    hang();
  }

  if (!kvmmap(PROG_STACK_PAGE, prog_stack, 0x07)) {
    kprintferr("Failed to map program stack.", 0x0F);
    hang();
  }

  memset((void *)PROG_STACK_PAGE, 0, 4096);

  kprintinfo("Switching to ring3 shell (/bin/sh)...", 0x0F);

  thread_t *user = spawn_thread((void (*)(void))USER_CODE_VIRT);

  if (!user) {
    kprintferr("Failed to spawn shell thread.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }

  user->context->cs = 0x18 | 3;
  user->context->ss = 0x20 | 3;
  user->context->rsp = USER_STACK_TOP;

  current_thread = user;
  tss.rsp0 = ((uint64_t)user->kernel_stack + 4096);
  switch_context(user->context);

  for (;;) {
    hlt();
  }
}
