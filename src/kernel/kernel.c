#include "io.h"
#include "utils.h"
#include "mem.h"
#include "idt.h"

void klog_ram_data() {
  char buf[21];
  size_t count = *MMAP_COUNT;

  if (!ki64toa(count, buf, sizeof(buf))) {
    kprintfail("Cannot convert memory map count.", 0x0F);
    return;
  }

  char msg[64];
  size_t i = 0;

  for (; buf[i]; i++)
    msg[i] = buf[i];

  char tmp[] = " memory map entries.";
  
  for (size_t j = 0; tmp[j]; j++, i++)
    msg[i] = tmp[j];

  msg[i] = '\0';
  kprintinfo(msg, 0x0F);

  size_t total = 0;

  for (size_t i = 0; i < count; i++) {
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
  
  for (size_t j = 0; ram[j]; j++, i++)
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

  uint64_t cr3;
  __asm__ volatile ("mov %%cr3, %0" : "=r"(cr3));

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

__attribute__((section(".text.entry")))
void kmain() {
  kprintsucc("Nyvela kernel started.", 0x0F);

  klog_ram_data();

  kprintinfo("Initializing PMM...", 0x0F);
  
  if (!kpmm_init()) {
    kprintferr("PMM initialization failed.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }
  
  kprintsucc("PMM ready.", 0x0F);

  ktest_palloc();

  kprintinfo("Initializing VMM...", 0x0F);
  
  if (!kvmm_init()) {
    kprintferr("VMM initialization failed.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }
  
  kprintsucc("VMM ready.", 0x0F);

  ktest_palloc();
  ktest_vmmap();

  kprintinfo("Initializing kmalloc...", 0x0F);
  
  if (!kmalloc_init()) {
    kprintferr("kmalloc initialization failed.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }
  
  kprintsucc("kmalloc ready.", 0x0F);

  ktest_kmalloc();

  kprintinfo("Initializing IDT...", 0x0F);
  
  if (!kidt_init()) {
    kprintferr("IDT initialization failed.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }
  
  kprintsucc("IDT ready.", 0x0F);

  __asm__ volatile ("cli\nhlt");
}
