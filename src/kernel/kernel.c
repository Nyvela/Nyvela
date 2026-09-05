#include "io.h"

__attribute__((section(".text.entry")))
void kmain() {
  kprintsucc("Entered Kernel in long mode.", 0x0F);

  __asm__ volatile ("cli\nhlt");
}
