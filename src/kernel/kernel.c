#include "../../include/nyvela/drivers/video/console/console.h"
#include "../../include/nyvela/lib/utils.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/mm/vmm.h"
#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/arch/x86_64/idt.h"
#include "../../include/nyvela/arch/x86_64/apic/apic.h"
#include "../../include/nyvela/arch/x86_64/asm/cpu.h"
#include "../../include/nyvela/thread/thread.h"

extern void main();

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
  // realloc(NULL, n) must behave like malloc.
  void* a = krealloc(NULL, 64);

  if (!a) {
    kprintferr("krealloc NULL failed.", 0x0F);
    return;
  }

  for (uint64_t i = 0; i < 64; i++) {
    ((uint8_t*)a)[i] = (uint8_t)(i & 0xFF);
  }

  // Growing must preserve existing data.
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

  // Shrinking must preserve the prefix.
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

  // Size 0 must free and return NULL.
  void* d = krealloc(c, 0);

  if (d != NULL) {
    kprintferr("krealloc zero should return NULL.", 0x0F);
    kfree(d);
    return;
  }

  // Oversize must fail without touching the old block.
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

  uint64_t cr3 = cpu_read_cr3();

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
    cpu_halt();
  }
  
  kprintsucc("PMM ready.", 0x0F);

  ktest_palloc();

  kprintinfo("Initializing VMM...", 0x0F);
  
  if (!kvmm_init()) {
    kprintferr("VMM initialization failed.", 0x0F);
    cpu_halt();
  }
  
  kprintsucc("VMM ready.", 0x0F);

  ktest_palloc();
  ktest_vmmap();

  kprintinfo("Initializing kmalloc...", 0x0F);
  
  if (!kmalloc_init()) {
    kprintferr("kmalloc initialization failed.", 0x0F);
    cpu_halt();
  }
  
  kprintsucc("kmalloc ready.", 0x0F);

  ktest_kmalloc();
  ktest_krealloc();

  kprintinfo("Initializing IDT...", 0x0F);
  
  if (!kidt_init()) {
    kprintferr("IDT initialization failed.", 0x0F);
    cpu_halt();
  }
  
  kprintsucc("IDT ready.", 0x0F);  
  
  if (!kenable_lapic()) {
    kprintferr("Failed to enable LAPIC.", 0x0F);
    cpu_halt();
  }

  kprintsucc("LAPIC enabled.", 0x0F);
  
  ksetup_lapic_timer();
  
  kprintsucc("LAPIC timer setup complete.", 0x0F);
  
  cpu_sti();

  for (;;) {
    cpu_hlt();
  }
}
