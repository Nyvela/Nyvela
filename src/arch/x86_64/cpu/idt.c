#include "../../../../include/nyvela/arch/x86_64/idt.h"
#include "../../../../include/nyvela/drivers/video/console/console.h"
#include "../../../../include/nyvela/scheduler/scheduler.h"
#include "../../../../include/nyvela/arch/x86_64/context.h"
#include "../../../../include/nyvela/arch/x86_64/apic/apic.h"
#include "../../../../include/nyvela/arch/x86_64/asm/cpu.h"
#include "../../../../include/nyvela/arch/x86_64/asm/desc.h"
#include "../../../../include/nyvela/arch/x86_64/asm/io.h"
#include "../../../../include/nyvela/arch/x86_64/pic/pic.h"
#include "../../../../include/nyvela/arch/x86_64/pit/pit.h"
#include "../../../../include/nyvela/syscall/syscall.h"

extern void isr_de();
extern void isr_pf();
extern void isr_gp();
extern void isr_ud();
extern void isr_df();
extern void isr_lapic_timer();
extern void isr_pit();
extern void isr_kbd();
extern void isr_syscall();

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

void idt_set_gate_user(int vector, void (*handler)(void)) {
  uint64_t addr = (uint64_t)handler;

  IDT[vector].offset_low = addr & 0xFFFF;
  IDT[vector].selector = 0x28;
  IDT[vector].ist = 0;
  IDT[vector].type_attr = 0xEE;
  IDT[vector].offset_mid = (addr >> 16) & 0xFFFF;
  IDT[vector].offset_high = (addr >> 32) & 0xFFFFFFFF;
  IDT[vector].zero = 0;
}

void lidt(idtr_t idtr) {
  desc_lidt(&idtr);
}

void isr_lapic_timer_handler() {
  klapic_eoi();
  scheduler_tick();
}

void isr_pit_handler() {
  pit_ticks++;
  kpic_eoi(0);
}

bool kidt_init() {
  idt_set_gate(0x00, isr_de);
  idt_set_gate(0x06, isr_ud);
  idt_set_gate(0x08, isr_df);
  idt_set_gate(0x0E, isr_pf);
  idt_set_gate(0x0D, isr_gp);
  idt_set_gate(0x20, isr_pit);
  idt_set_gate(0x21, isr_kbd);
  idt_set_gate(LAPIC_TIMER_VECTOR, isr_lapic_timer);
  idt_set_gate_user(SYSCALL_VECTOR, isr_syscall);

  lidt((idtr_t){
    .limit = sizeof(IDT) - 1,
    .base = (uint64_t)&IDT
  });

  return true;
}
