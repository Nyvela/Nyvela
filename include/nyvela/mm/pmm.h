#ifndef NYVPMM_H
#define NYVPMM_H

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

bool kpmm_init();

void* kpalloc();
void kpfree(void* page);

extern uint64_t* MMAP_COUNT;
extern e820_entry_t* MMAP_ENTRIES;
extern uint8_t* BITMAP;
extern uint64_t BITMAP_SIZE;
extern uint64_t FRAME_COUNT;

#endif // NYVPMM_H
