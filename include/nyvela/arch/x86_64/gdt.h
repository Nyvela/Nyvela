#ifndef NYVGDT_H
#define NYVGDT_H

#include <stdint.h>
#include <stddef.h>

#define GDT_NULL_SEGMENT 0x00
#define GDT_KCODE_SEGMENT 0x08
#define GDT_KDATA_SEGMENT 0x10
#define GDT_UCODE_SEGMENT 0x18
#define GDT_UDATA_SEGMENT 0x20
#define GDT_KCODE_LM_SEGMENT 0x28
#define GDT_TSS_SEGMENT 0x30

typedef struct tss_t {
  uint32_t reserved0;

  uint64_t rsp0;
  uint64_t rsp1;
  uint64_t rsp2;

  uint64_t reserved1;

  uint64_t ist1;
  uint64_t ist2;
  uint64_t ist3;
  uint64_t ist4;
  uint64_t ist5;
  uint64_t ist6;
  uint64_t ist7;

  uint64_t reserved2;

  uint16_t reserved3;
  uint16_t iomap_base;
} __attribute__((packed)) tss_t;

typedef struct gdtr_t {
  uint16_t limit;
  uint64_t base;
} __attribute__((packed)) gdtr_t;

_Static_assert(sizeof(gdtr_t) == 10, "gdtr_t must be 10 bytes");
_Static_assert(offsetof(gdtr_t, limit) == 0, "gdtr limit offset");
_Static_assert(offsetof(gdtr_t, base) == 2, "gdtr base offset"); 

bool ktss_init();
void tss_set_rsp0(uint64_t rsp0);

#endif // NYVGDT_H
