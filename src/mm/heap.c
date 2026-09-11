#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/mm/pmm.h"

#include <stddef.h>

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
