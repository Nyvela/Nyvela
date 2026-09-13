#ifndef NYVPIT_H
#define NYVPIT_H

#include <stdint.h>

#define PIT_FREQUENCY 1193182

#define PIT_CHANNEL0 0x40
#define PIT_COMMAND 0x43

#define PIT_CH0 0x00
#define PIT_LOHI 0x30
#define PIT_MODE2 0x04

extern volatile uint64_t pit_ticks;

void kpit_set_freq(uint32_t freq);
void kpit_wait_ms(uint64_t ms);

#endif // NYVPIT_H
