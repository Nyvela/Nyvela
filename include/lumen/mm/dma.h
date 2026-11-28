#ifndef LUMDMA_H
#define LUMDMA_H

#include <stdint.h>

#define DMA_VIRT_BASE 0x80000000ULL
#define DMA_BITMAP_SIZE 128

#define DMA_PAGE_AMOUNT 16

bool kdma_init();

void* kdma_palloc();
void* kdma_pnalloc(uint64_t pages);

void kdma_pfree(void *page);
void kdma_pnfree(void *pages_start);

extern uint8_t DMA_BITMAP[];
extern uint64_t DMA_PAGES;

#endif // LUMDMA_H
