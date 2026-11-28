#ifndef LUMPIC_H
#define LUMPIC_H

#include <stdint.h>

void kpic_init();
void kpic_eoi(uint8_t irq);
void kpic_disable();
void kpic_enable_irq(uint8_t irq);
void kpic_disable_irq(uint8_t irq);

#endif // LUMPIC_H
