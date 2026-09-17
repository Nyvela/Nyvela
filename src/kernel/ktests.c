#include "../../include/nyvela/ktests/ktests.h"
#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/mm/vmm.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/fs/vfs.h"
#include "../../include/nyvela/drivers/video/console/console.h"
#include "../../include/nyvela/lib/utils.h"
#include "../../include/nyvela/arch/x86_64/asm/cpu.h"

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
