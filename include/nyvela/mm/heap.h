#ifndef NYVHEAP_H
#define NYVHEAP_H

#include <stdint.h>

typedef struct block_t {
  uint64_t size;
  bool is_used;
  struct block_t* next;
} block_t;

bool kmalloc_init();

void* kmalloc(uint64_t size);
void kfree(void* ptr);

#endif // NYVHEAP_H
