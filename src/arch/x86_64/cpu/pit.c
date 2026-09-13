#include "../../../../include/nyvela/arch/x86_64/pit/pit.h"
#include "../../../../include/nyvela/arch/x86_64/asm/io.h"

volatile uint64_t pit_ticks = 0;

void kpit_set_freq(uint32_t freq) {
  uint16_t divisor = PIT_FREQUENCY / freq;

  outb(PIT_COMMAND, PIT_CH0 | PIT_LOHI | PIT_MODE2);

  outb(PIT_CHANNEL0, divisor & 0xFF);
  outb(PIT_CHANNEL0, (divisor >> 8) & 0xFF);
}

void kpit_wait_ms(uint64_t ms) {
  uint64_t target = pit_ticks + ms;

  while (pit_ticks < target) {
    __asm__ volatile ("hlt");
  }
}
