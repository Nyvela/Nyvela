#ifndef NYV_KBD_H
#define NYV_KBD_H

#include <stdint.h>
#include <stdbool.h>

void kbd_init(void);

void kbd_irq_handler(void);

int kbd_getc(void);
bool kbd_empty(void);

#endif // NYV_KBD_H
