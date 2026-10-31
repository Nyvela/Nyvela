#include "../../include/nyvela/mm/dma.h"
#include "../../include/nyvela/mm/pmm.h"

uint8_t DMA_BITMAP[BITMAP_SIZE];
uint64_t DMA_PAGES = 0;

bool kdma_init() {
  memset(DMA_BITMAP, 0, BITMAP_SIZE);

  DMA_PAGES = (uint64_t)kpalloc_contiguous(PAGE_AMOUNT);

  if (!DMA_PAGES) {
    return false;
  }

  return true;
}

void* kdma_palloc() {
  for (uint64_t i = 0; i < DMA_PAGE_AMOUNT; i++) {
    uint8_t byte = i / 8;
    uint8_t mask = 1u << (i % 8);

    if (!(DMA_BITMAP[byte] & mask)) {
      DMA_BITMAP[byte] |= mask;
      return (void*)(DMA_PAGES + (4096 * i));
    }
  }

  return NULL;
}

void* kdma_pnalloc(uint64_t pages) {
}

void kdma_pfree(void *page) {
  uint64_t addr = (uint64_t)page;

  if (addr < DMA_PAGES || addr >= DMA_PAGES + (DMA_PAGE_AMOUNT * 4096)) {
    return;
  }

  uint64_t offset = addr - DMA_PAGES;
  
  if (offset % 4096 != 0) {
    return;
  }

  uint64_t n = offset / 4096;

  uint8_t byte = n / 8;
  uint8_t mask = 1u << (n % 8);

  DMA_BITMAP[byte] &= ~mask;
}

void kdma_pnfree(void *pages_start) {
}
