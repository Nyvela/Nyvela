#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/lib/utils.h"
#include "../../include/nyvela/drivers/video/console/console.h"
#include "../../include/nyvela/arch/x86_64/asm/cpu.h"

extern uint8_t kernel_end;

uint64_t* MMAP_COUNT = (uint64_t*)0xCFFE;
e820_entry_t* MMAP_ENTRIES = (e820_entry_t*)0xD000;
uint8_t* BITMAP = (uint8_t*)0x10000;
uint64_t BITMAP_SIZE = 0;
uint64_t FRAME_COUNT = 0;

bool kpmm_init() {
  uint64_t end_addr = 0;
  uint64_t mmap_entry_count = *MMAP_COUNT;

  for (uint64_t i = 0; i < mmap_entry_count; i++) {
    uint64_t addr = MMAP_ENTRIES[i].base_addr + MMAP_ENTRIES[i].length_in_bytes;

    if (addr > end_addr) {
      end_addr = addr;
    }
  }
  
  FRAME_COUNT = (end_addr + 0xFFF) / 0x1000;
  BITMAP_SIZE = (FRAME_COUNT + 7) / 8;

  memset(BITMAP, BITMAP_RESERVED, BITMAP_SIZE);

  for (uint64_t i = 0; i < mmap_entry_count; i++) {
    if (MMAP_ENTRIES[i].type == E820_USABLE) {
      const uint64_t base = MMAP_ENTRIES[i].base_addr;
      const uint64_t end = base + MMAP_ENTRIES[i].length_in_bytes;
      
      const uint64_t first_page = (base + 4095) / 4096;
      const uint64_t last_page = end / 4096;

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
      uint64_t phys = page * 0x1000;

      if (phys >= 0x400000 && phys < 0x600000) continue;

      BITMAP[page / 8] &= ~(BITMAP_FREE << (page % 8));
      return (void*)phys;
    }
  }

  return NULL;
}

void* kpalloc_top(void) {
  const uint64_t first_page = ((uint64_t)&kernel_end + 4095) / 4096;
  
  for (uint64_t page = FRAME_COUNT; page-- > first_page;) {
    if (BITMAP[page / 8] & (BITMAP_FREE << (page % 8))) {
      uint64_t phys = page * 0x1000;
      if (phys >= 0x400000 && phys < 0x600000) continue;
      BITMAP[page / 8] &= ~(BITMAP_FREE << (page % 8));
      return (void*)phys;
    }
  }
  
  return NULL;
}

void* kpalloc_contiguous(uint64_t pages) {
  if (pages == 0) return NULL;
  if (pages == 1) return kpalloc();
  
  const uint64_t first_page = ((uint64_t)&kernel_end + 4095) / 4096;
  const uint64_t last_page = FRAME_COUNT;
  
  for (uint64_t page = first_page; page + pages <= last_page; page++) {
    bool ok = true;
 
    for (uint64_t i = 0; i < pages; i++) {
      uint64_t phys = (page + i) * 0x1000;
      if (phys >= 0x400000 && phys < 0x600000) {
        ok = false;
        break;
      }
  
      if (!(BITMAP[(page + i) / 8] & (BITMAP_FREE << ((page + i) % 8)))) { 
        ok = false;
        break;
      }  
    }
    
    if (!ok) continue;
    
    for (uint64_t i = 0; i < pages; i++) {
      BITMAP[(page + i) / 8] &= ~(BITMAP_FREE << ((page + i) % 8));
    
    }
    
    return (void*)(page * 0x1000);
  }

  return NULL;
}

void kpfree(void* page) {
  if (!page) return; 
  
  uint64_t page_addr = (uint64_t)page;
  
  if (page_addr % 0x1000 != 0) {
    kprintferr("Invalid kpfree.", 0x0F);
    hang();
  }

  uint64_t frame = page_addr / 0x1000;

  if (frame >= FRAME_COUNT) {
    kprintferr("Invalid kpfree.", 0x0F);
    hang();
  }

  uint8_t is_used = !(BITMAP[frame / 8] & (BITMAP_FREE << (frame % 8)));

  if (!is_used) {
    kprintferr("Invalid kpfree.", 0x0F);
    hang();
  }

  BITMAP[frame / 8] |= (BITMAP_FREE << (frame % 8));
}

void kpfree_contiguous(void* page, uint64_t pages) {
  if (!page || pages == 0) return;
  for (uint64_t i = 0; i < pages; i++) {
    kpfree((void*)((uint64_t)page + i*0x1000));
  }
}
