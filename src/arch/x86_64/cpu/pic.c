#include "../../../../include/nyvela/arch/x86_64/pic/pic.h"
#include "../../../../include/nyvela/arch/x86_64/asm/io.h"

void kpic_init() {
  outb(0x20, 0x11); // initialize ICW1 master
  outb(0xA0, 0x11); // initialize ICW1 slave

  outb(0x21, 0x20); // master IRQs remapped to 0x20-0x27
  outb(0xA1, 0x28); // slave IRQs remapped to 0x28-0x2F

  outb(0x21, 0x04); // master has slave on IRQ2
  outb(0xA1, 0x02); // slave identity is 2
  
  outb(0x21, 0x01); // 8086 mode
  outb(0xA1, 0x01); // 8086 mode
  
  outb(0x21, 0xFE); // IRQ0 enabled, IRQ1-7 masked
  outb(0xA1, 0xFF); // IRQ8-15 masked
}

void kpic_eoi(uint8_t irq) {
  if (irq >= 8) {
    outb(0xA0, 0x20);
  }

  outb(0x20, 0x20);
}
