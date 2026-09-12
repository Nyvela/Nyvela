#include "../../../../../include/nyvela/arch/x86_64/asm/out.h"

void outb(uint16_t port, uint8_t value) {
  __asm__ volatile (
    "outb %0, %1" : : "a"(value), "Nd"(port)
  );
}
