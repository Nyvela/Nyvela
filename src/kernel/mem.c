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

void* kmalloc(size_t size) {
  (void)size;
  return NULL;
}

void kfree(void *ptr) {
  (void)ptr;
}
