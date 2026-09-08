#include "io.h"
#include "utils.h"
#include "mem.h"

void klog_ram_data() {
  char buf[21];
  size_t count = *MMAP_COUNT;
  
  if (!ki64toa(count, buf, sizeof(buf))) {
    kprintfail("Cannot convert MMAP_COUNT to string.", 0x0F);
  } else {
    char tmp[] = " memory map entries available.";
    char msg[21 + sizeof(tmp)];

    size_t i = 0;

    for (; buf[i]; i++)
        msg[i] = buf[i];

    for (size_t j = 0; tmp[j]; j++, i++)
        msg[i] = tmp[j];

    msg[i] = '\0';

    kprintinfo(msg, 0x0F);
  }
  
  size_t total = 0;

  for (size_t i = 0; i < count; i++) {
    if (MMAP_ENTRIES[i].type == E820_USABLE) {
      total += MMAP_ENTRIES[i].length_in_bytes;
    }
  }

  if (!ki64toa(total, buf, sizeof(buf))) {
    kprintfail("Cannot convert total available memory to string.", 0x0F);
  } else {
    char tmp[] = " bytes of RAM available.";
    char msg[21 + sizeof(tmp)];

    size_t i = 0;

    for (; buf[i]; i++)
        msg[i] = buf[i];

    for (size_t j = 0; tmp[j]; j++, i++)
        msg[i] = tmp[j];

    msg[i] = '\0';

    kprintinfo(msg, 0x0F);
  }
}

void ktest_kmalloc() {
  void* a = kmalloc(64);

  if (!a) {
    kprintferr("kmalloc failed.", 0x0F);
    return;
  }

  kprintsucc("kmalloc allocated 64 bytes.", 0x0F);

  *(uint64_t*)a = 0x1111222233334444;

  if (*(uint64_t*)a != 0x1111222233334444) {
    kprintferr("kmalloc memory write failed.", 0x0F);
    return;
  }

  void* b = kmalloc(128);

  if (!b) {
    kprintferr("Second kmalloc failed.", 0x0F);
    return;
  }

  kprintsucc("Second kmalloc succeeded.", 0x0F);

  if (a == b) {
    kprintferr("kmalloc returned overlapping blocks.", 0x0F);
    return;
  }

  kfree(a);
  kprintsucc("kfree succeeded.", 0x0F);

  void* c = kmalloc(32);

  if (!c) {
    kprintferr("kmalloc failed to reuse freed block.", 0x0F);
    return;
  }

  kprintsucc("kmalloc reused freed memory.", 0x0F);

  kfree(b);
  kfree(c);

  kprintsucc("kmalloc/kfree test passed!", 0x0F);
}

void ktest_palloc() {
  void* mem = kpalloc();

  if (!mem) {
    kprintferr("Failed to allocate page.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }

  char buf[21];

  if (!ki64toa((uint64_t)mem, buf, sizeof(buf))) {
    kprintfail("Failed to convert allocated memory address to string.", 0x0F);
  } else {
    // "Allocated memory address: " 
    char tmp[27 + sizeof(buf) + 1] = {0};

    memcpy(tmp, "Allocated memory address: ", 26);
    memcpy(tmp + 26, buf, 21);

    kprintinfo(tmp, 0x0F);
  }

  kpfree(mem);
  kprintinfo("Free'd allocated memory.", 0x0F);
}

void ktest_vmmap() {
  uint64_t phys = (uint64_t)kpalloc();
  uint64_t virt = 0x100000000;

  if (!phys) {
    kprintferr("Failed to allocate test page", 0x0F);
    return;
  }

  *(volatile uint64_t*)phys = 0x1111222233334444;

  if (!kvmmap(virt, phys, 0x03)) {
    kprintferr("kvmmap failed", 0x0F);
    return;
  }

  if (*(volatile uint64_t*)virt != 0x1111222233334444) {
    kprintferr("VMM mapping failed", 0x0F);
    return;
  }

  *(volatile uint64_t*)virt = 0xAAAABBBBCCCCDDDD;

  if (*(volatile uint64_t*)phys != 0xAAAABBBBCCCCDDDD) {
    kprintferr("VMM mapping failed", 0x0F);
    return;
  }

  kprintsucc("kvmmap works!", 0x0F);

  if (!kvmunmap(virt)) {
    kprintferr("kunmap failed", 0x0F);
    return;
  }

  uint64_t cr3;
  __asm__ volatile ("mov %%cr3, %0" : "=r"(cr3));

  uint64_t *pml4 = (uint64_t*)(cr3 & ~0xFFFULL);

  uint64_t pml4_index = (virt >> 39) & 0x1FFULL;
  uint64_t pdpt_index = (virt >> 30) & 0x1FFULL;
  uint64_t pd_index = (virt >> 21) & 0x1FFULL;
  uint64_t pt_index = (virt >> 12) & 0x1FFULL;

  uint64_t *pdpt = (uint64_t*)(pml4[pml4_index] & ~0xFFFULL);

  uint64_t *pd = (uint64_t*)(pdpt[pdpt_index] & ~0xFFFULL);

  uint64_t *pt = (uint64_t*)(pd[pd_index] & ~0xFFFULL);

  if (pt[pt_index] & 1) {
    kprintferr("kunmap failed: PTE still present", 0x0F);
    return;
  }

  kprintsucc("kunmap works!", 0x0F);
  kpfree((void*)phys);
}

__attribute__((section(".text.entry")))
void kmain() {
  kprintsucc("Entered Kernel in long mode.", 0x0F);

  klog_ram_data();
  
  kprintinfo("Initializing PMM...", 0x0F);

  if (!kpmm_init()) {
    kprintferr("Failed to initialize PMM.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }
  
  kprintsucc("Initialized PMM.", 0x0F);
  
  ktest_palloc();
  
  if (!kvmm_init()) {
    kprintferr("Failed to initialize VMM.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }

  kprintsucc("Initialized VMM.", 0x0F);

  ktest_palloc();
  ktest_vmmap();
  
  if (!kmalloc_init()) {
    kprintferr("Failed to initialize kmalloc.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }

  kprintsucc("Initialized kmalloc.", 0x0F);

  ktest_kmalloc();

  __asm__ volatile ("cli\nhlt");
}
