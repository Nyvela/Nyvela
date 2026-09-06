#include "mem.h"

size_t* MMAP_COUNT = (size_t*)0xCFFE;
e820_entry_t* MMAP_ENTRIES = (e820_entry_t*)0xD000;

void* kmalloc(size_t size) {
  (void)size;
  return NULL;
}

void kfree(void *ptr) {
  (void)ptr;
}
