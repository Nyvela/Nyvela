#include "../../../../include/nyvela/arch/x86_64/idt.h"
#include "../../../../include/nyvela/drivers/video/console/console.h"
#include "../../../../include/nyvela/scheduler/scheduler.h"
#include "../../../../include/nyvela/arch/x86_64/context.h"
#include "../../../../include/nyvela/arch/x86_64/apic/apic.h"

extern void isr_de();
extern void isr_pf();
extern void isr_gp();
extern void isr_lapic_timer();

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

void isr_lapic_timer_handler(context_t* ctx) {
  klapic_eoi();
  scheduler_tick(ctx);
}

bool kidt_init() {
  idt_set_gate(0x00, isr_de);
  idt_set_gate(0x0E, isr_pf);
  idt_set_gate(0x0D, isr_gp);
  idt_set_gate(0x20, isr_lapic_timer);

  lidt((idtr_t){
    .limit = sizeof(IDT) - 1,
    .base = (uint64_t)&IDT
  });

  return true;
}

