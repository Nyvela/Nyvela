#ifndef NYVMEM_H
#define NYVMEM_H

#include <stddef.h>
#include <stdint.h>

typedef enum e820_type_t {
  E820_USABLE = 1,
  E820_RESERVED,
  E820_ACPI_RECLAIMABLE,
  E820_ACPI_NVS,
  E820_BAD_MEM
} e820_type_t;

typedef enum BITMAP_TYPES {
  BITMAP_RESERVED,
  BITMAP_FREE,
} BITMAP_TYPES;

typedef struct e820_entry_t {
  uint64_t base_addr;
  uint64_t length_in_bytes;
  e820_type_t type;
  uint32_t extended_attributes;
} e820_entry_t;

_Static_assert(sizeof(e820_entry_t) == 24, "Invalid E820 entry size");

typedef struct block_t {
  uint64_t size;
  bool is_used;
  struct block_t* next;
} block_t;

bool kpmm_init();
bool kvmm_init();
bool kmalloc_init();

void* kpalloc();
void kpfree(void* page);

void* kmalloc(uint64_t size);
void kfree(void* ptr);

bool kvmmap(uint64_t virt, uint64_t phys, uint64_t flags);
bool kvmunmap(uint64_t virt);

extern size_t* MMAP_COUNT;
extern e820_entry_t* MMAP_ENTRIES;
extern uint8_t* BITMAP;
extern uint64_t BITMAP_SIZE;
extern uint64_t FRAME_COUNT;

#endif // NYVMEM_H
