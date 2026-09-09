#include "idt.h"
#include "io.h"

extern void isr_de();
extern void isr_pf();

idt_entry_t IDT[256] = {0};

void idt_set_gate(int vector, void (*handler)(void)) {
  uint64_t addr = (uint64_t)handler;

  IDT[vector].offset_low = addr & 0xFFFF;
  IDT[vector].selector = 0x28;
  IDT[vector].ist = 0;
  IDT[vector].type_attr = 0x8E;
  IDT[vector].offset_mid = (addr >> 16) & 0xFFFF;
  IDT[vector].offset_high = (addr >> 32) & 0xFFFFFFFF;
  IDT[vector].zero = 0;
}

void lidt(idtr_t idtr) {
  __asm__ volatile ("lidt %0" : : "m"(idtr));
}

void iretq() {
  __asm__ volatile ("iretq");
}

void isr_de_handler(void) {
  kprintferr("Division Error.", 0x0F);
  __asm__ volatile ("cli\nhlt");
}

void isr_pf_handler(void) {
  kprintferr("Page Fault.", 0x0F);
  __asm__ volatile ("cli\nhlt");
}

bool kidt_init() {
  idt_set_gate(0, isr_de);
  idt_set_gate(14, isr_pf);

  lidt((idtr_t){
    .limit = sizeof(IDT) - 1,
    .base = (uint64_t)&IDT
  });

  return true;
}

