#ifndef NYVIDT_H
#define NYVIDT_H

#include <stdint.h>
#include <stdbool.h>

typedef struct __attribute__((packed)) idt_entry_t {
  uint16_t offset_low;
  uint16_t selector;
  uint8_t  ist;
  uint8_t  type_attr;
  uint16_t offset_mid;
  uint32_t offset_high;
  uint32_t zero;
} idt_entry_t;

typedef struct __attribute__((packed)) idtr_t {
  uint16_t limit;
  uint64_t base;
} idtr_t;

_Static_assert(sizeof(idt_entry_t) == 16, "idt_entry_t must be 16 bytes");
_Static_assert(sizeof(idtr_t) == 10, "idtr_t must be 10 bytes");

extern idt_entry_t IDT[];

bool kidt_init();

void isr_de_handler(void);
void isr_pf_handler(void);

#endif
