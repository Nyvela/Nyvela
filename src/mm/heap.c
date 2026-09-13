#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/lib/utils.h"

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
  if (size == 0 || size >= (4096 - sizeof(block_t))) {
    return NULL;
  }

  uint64_t aligned_size = (size + 15) & ~15ULL;
  
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
        return (void*)((uint8_t*)new_block + sizeof(block_t));
      }

      // Fits but no room for a new header + minimal payload: use whole block.
      if (block->size < aligned_size + sizeof(block_t) + 16) {
        block->is_used = true;
        return (void*)((uint8_t*)block + sizeof(block_t));
      }
      
      block_t* new_block = (block_t*)((uint8_t*)block + aligned_size + sizeof(block_t));

      *new_block = (block_t){
        .size = block->size - sizeof(block_t) - aligned_size,
        .is_used = false,
        .next = block->next
      };

      block->size = aligned_size;
      block->next = new_block;
      block->is_used = true;

      return (void*)((uint8_t*)block + sizeof(block_t));
    }
  }

  return NULL;
}

void* krealloc(void* ptr, uint64_t new_size) {
  if (!ptr) {
    return kmalloc(new_size);
  }

  if (new_size == 0) {
    kfree(ptr);
    return NULL;
  }

  if (new_size >= (4096 - sizeof(block_t))) {
    return NULL;
  }

  block_t* old_block = (block_t*)((uint8_t*)ptr - sizeof(block_t));
  uint64_t old_size = old_block->size;

  uint64_t aligned_new = (new_size + 15) & ~15ULL;

  // Shrinking or same size: keep in place, data already fits.
  if (aligned_new <= old_size) {
    return ptr;
  }

  void* new_ptr = kmalloc(new_size);

  if (!new_ptr) {
    return NULL;
  }

  memcpy(new_ptr, ptr, old_size);
  kfree(ptr);

  return new_ptr;
}

void kfree(void *ptr) {
  if (!ptr) return;

  block_t* block = (block_t*)((uint8_t*)ptr - sizeof(block_t));
  block->is_used = false;
}
