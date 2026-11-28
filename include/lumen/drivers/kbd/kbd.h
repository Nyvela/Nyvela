#ifndef LUM_KBD_H
#define LUM_KBD_H

#include <stdint.h>
#include <stdbool.h>

void kbd_init(void);

void kbd_irq_handler(void);

int kbd_getc(void);
bool kbd_empty(void);

#endif // LUM_KBD_H
