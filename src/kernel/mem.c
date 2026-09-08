#include "mem.h"
#include "utils.h"
#include "io.h"

extern uint8_t kernel_end;

size_t* MMAP_COUNT = (size_t*)0xCFFE;
e820_entry_t* MMAP_ENTRIES = (e820_entry_t*)0xD000;
uint8_t* BITMAP = (uint8_t*)0x10000;
uint64_t BITMAP_SIZE = 0;
uint64_t FRAME_COUNT = 0;

bool kpmm_init() {
  uint64_t end_addr = 0;
  uint64_t mmap_entry_count = *MMAP_COUNT;

  for (size_t i = 0; i < mmap_entry_count; i++) {
    uint64_t addr = MMAP_ENTRIES[i].base_addr + MMAP_ENTRIES[i].length_in_bytes;

    if (addr > end_addr) {
      end_addr = addr;
    }
  }
  
  FRAME_COUNT = (end_addr + 0xFFF) / 0x1000;
  BITMAP_SIZE = (FRAME_COUNT + 7) / 8;

  memset(BITMAP, BITMAP_RESERVED, BITMAP_SIZE);

  for (size_t i = 0; i < mmap_entry_count; i++) {
    if (MMAP_ENTRIES[i].type == E820_USABLE) {
      const uint64_t base = MMAP_ENTRIES[i].base_addr;
      const uint64_t end = base + MMAP_ENTRIES[i].length_in_bytes;
      
      const uint64_t first_page = (base + 4095) / 4096; // align up
      const uint64_t last_page = end / 4096; // align down

      for (uint64_t page = first_page; page < last_page; page++) {
        BITMAP[page / 8] |= BITMAP_FREE << (page % 8);
      }
    }
  }
  
  const uint64_t first_page = 0;
  const uint64_t last_page = ((uint64_t)&kernel_end + 4095) / 4096;

  for (uint64_t page = first_page; page < last_page; page++) {
    BITMAP[page / 8] &= ~(BITMAP_FREE << (page % 8));
  }

  return true;
}

void* kpalloc() {
  const uint64_t first_page = ((uint64_t)&kernel_end + 4095) / 4096;
  const uint64_t last_page = FRAME_COUNT;

  for (uint64_t page = first_page; page < last_page; page++) {
    if (BITMAP[page / 8] & (BITMAP_FREE << (page % 8))) {
      BITMAP[page / 8] &= ~(BITMAP_FREE << (page % 8));
      return (void*)(page * 0x1000);
    }
  }

  return NULL;
}

void kpfree(void* page) {
  if (!page) return; 
  
  uint64_t page_addr = (uint64_t)page;
  
  if (page_addr % 0x1000 != 0) {
    kprintferr("Invalid kpfree.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }

  uint64_t frame = page_addr / 0x1000;

  if (frame >= FRAME_COUNT) {
    kprintferr("Invalid kpfree.", 0x0F);
    __asm__ volatile("cli\nhlt");
  }

  uint8_t is_used = !(BITMAP[frame / 8] & (BITMAP_FREE << (frame % 8)));

  if (!is_used) {
    kprintferr("Invalid kpfree.", 0x0F);
    __asm__ volatile("cli\nhlt");
  }

  BITMAP[frame / 8] |= (BITMAP_FREE << (frame % 8));
}

bool kvmm_init() {
  uint64_t cr3;
  __asm__ volatile ("mov %%cr3, %0" : "=r"(cr3));

  uint64_t pml4_phys = cr3 & ~0xFFFULL;
  uint64_t *pml4 = (uint64_t*)pml4_phys;

  uint64_t pdpt_phys = pml4[0] & ~0xFFFULL;
  uint64_t *pdpt = (uint64_t*)pdpt_phys;
  
  uint64_t pd_phys = pdpt[0] & ~0xFFFULL;
  uint64_t *pd = (uint64_t*)pd_phys;

  for (uint64_t i = 0; i < 512; i++) {   
    uint64_t pt_phys = (uint64_t)kpalloc();

    if (!pt_phys) {
      kprintferr("Failed to allocate memory for PT entry.", 0x0F);
      return false;
    }

    uint64_t* pt = (uint64_t*)pt_phys;

    for (uint64_t j = 0; j < 512; j++) {
      uint64_t phys = (i * 0x200000) + (j * 0x1000);
      pt[j] = phys | 0x03;
    }

    pd[i] = pt_phys | 0x03;
  }

  __asm__ volatile ("mov %0, %%cr3" :: "r"(cr3) : "memory");

  return true;
}

bool kvmunmap(uint64_t virt) { 
  uint64_t cr3;
  __asm__ volatile ("mov %%cr3, %0" : "=r"(cr3));
  
  uint64_t pml4_index = (virt >> 39) & 0x1FFULL;
  uint64_t pdpt_index = (virt >> 30) & 0x1FFULL;
  uint64_t pd_index = (virt >> 21) & 0x1FFULL;
  uint64_t pt_index = (virt >> 12) & 0x1FFULL;
  
  uint64_t pml4_phys = cr3 & ~0xFFFULL;
  uint64_t *pml4 = (uint64_t*)pml4_phys;

  uint64_t pml4e = pml4[pml4_index];
  
  uint64_t pdpt_phys;
  uint64_t *pdpt;

  if (!(pml4e & 1)) {
    return false;
  } else {
    pdpt_phys = pml4e & ~0xFFFULL;
    pdpt = (uint64_t*)pdpt_phys;
  }
  
  uint64_t pdpte = pdpt[pdpt_index];
  
  uint64_t pd_phys;
  uint64_t *pd;

  if (!(pdpte & 1)) {
    return false;
  } else {
    pd_phys = pdpte & ~0xFFFULL;
    pd = (uint64_t*)pd_phys;
  }
  
  uint64_t pde = pd[pd_index];

  uint64_t pt_phys;
  uint64_t *pt;
  
  if (!(pde & 1)) {
    return false;
  } else {
    pt_phys = pde & ~0xFFFULL;
    pt = (uint64_t*)pt_phys;
  }

  pt[pt_index] = 0;
  __asm__ volatile ("mov %0, %%cr3" :: "r"(cr3) : "memory");

  return true;
}

bool kvmmap(uint64_t virt, uint64_t phys, uint64_t flags) { 
  uint64_t cr3;
  __asm__ volatile ("mov %%cr3, %0" : "=r"(cr3));
  
  uint64_t pml4_index = (virt >> 39) & 0x1FFULL;
  uint64_t pdpt_index = (virt >> 30) & 0x1FFULL;
  uint64_t pd_index = (virt >> 21) & 0x1FFULL;
  uint64_t pt_index = (virt >> 12) & 0x1FFULL;
  
  uint64_t pml4_phys = cr3 & ~0xFFFULL;
  uint64_t *pml4 = (uint64_t*)pml4_phys;

  uint64_t pml4e = pml4[pml4_index];
  
  uint64_t pdpt_phys;
  uint64_t *pdpt;

  if (!(pml4e & 1)) {
    pdpt_phys = (uint64_t)kpalloc();

    if (!pdpt_phys) {
      kprintferr("Failed to allocate PDPT entry.", 0x0F);
      __asm__ volatile ("cli\nhlt");
    }
     
    pdpt = (uint64_t*)pdpt_phys;

    for (uint64_t i = 0; i < 512; i++) {
      pdpt[i] = 0;
    }

    pml4[pml4_index] = pdpt_phys | 0x03;
  } else {
    pdpt_phys = pml4e & ~0xFFFULL;
    pdpt = (uint64_t*)pdpt_phys;
  }
  
  uint64_t pdpte = pdpt[pdpt_index];
  
  uint64_t pd_phys;
  uint64_t *pd;

  if (!(pdpte & 1)) {
    pd_phys = (uint64_t)kpalloc();

    if (!pd_phys) {
      kprintferr("Failed to allocate PD entry.", 0x0F);
      __asm__ volatile ("cli\nhlt");
    }
     
    pd = (uint64_t*)pd_phys;

    for (uint64_t i = 0; i < 512; i++) {
      pd[i] = 0;
    }

    pdpt[pdpt_index] = pd_phys | 0x03;
  } else {
    pd_phys = pdpte & ~0xFFFULL;
    pd = (uint64_t*)pd_phys;
  }
  
  uint64_t pde = pd[pd_index];

  uint64_t pt_phys;
  uint64_t *pt;
  
  if (!(pde & 1)) {
    pt_phys = (uint64_t)kpalloc();

    if (!pt_phys) {
      kprintferr("Failed to allocate PD entry.", 0x0F);
      __asm__ volatile ("cli\nhlt");
    }
     
    pt = (uint64_t*)pt_phys;

    for (uint64_t i = 0; i < 512; i++) {
      pt[i] = 0;
    }

    pd[pd_index] = pt_phys | 0x03;
  } else {
    pt_phys = pde & ~0xFFFULL;
    pt = (uint64_t*)pt_phys;
  }

  pt[pt_index] = (phys & ~0xFFFULL) | flags;
  __asm__ volatile ("mov %0, %%cr3" :: "r"(cr3) : "memory");

  return true;
}

block_t* CURRENT_BLOCK;

bool kmalloc_init() {
  void* page = kpalloc();
  
  if (!page) return false;

  CURRENT_BLOCK = (block_t*)page;
  
  *CURRENT_BLOCK = (block_t){
    .size = 4096 - sizeof(block_t),
    .is_used = false,
    .next = NULL
  };

  return true;
}

void* kmalloc(uint64_t size) {
  if (size <= 0 || size >= (4096 - sizeof(block_t))) {
    return NULL;
  }

  uint64_t aligned_size = (size + 15) & ~15;
  
  for (block_t *block = CURRENT_BLOCK; block; block = block->next) {
    if (!block->is_used) {
      if (block->size < aligned_size) {
        void* page = kpalloc(); // 4 KiB

        if (!page) {
          return NULL;
        }
        
        block_t *new_block = (block_t*)page;
        
        *new_block = (block_t){
          .size = 4096 - sizeof(block_t),
          .is_used = true,
          .next = block->next
        };

        block->next = new_block;
        return (((void*)new_block) + sizeof(block_t));
      }
      
      block_t* new_block = (block_t*)(((void*)block) + aligned_size + sizeof(block_t));

      *new_block = (block_t){
        .size = block->size - sizeof(block_t) - aligned_size,
        .is_used = false,
        .next = block->next
      };

      block->size = aligned_size;
      block->next = new_block;
      block->is_used = true;

      return (((void*)block) + sizeof(block_t));
    }
  }

  return NULL;
}

void kfree(void *ptr) {
  if (!ptr) return;

  block_t* block = (ptr - sizeof(block_t));
  block->is_used = false;
}
