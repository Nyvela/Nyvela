#ifndef NYV_KBD_H
#define NYV_KBD_H

#include <stdint.h>
#include <stdbool.h>

// PS/2 keyboard, scancode set 1 (controller-translated), US layout.
// IRQ1 -> IDT vector 0x21. 256-byte ring buffer; drops on overflow.
//
// v1 limits: no arrows/numpad/extended keys, no Ctrl/Alt combos.

void kbd_init(void);

// IRQ entry (called from isr_kbd). Reads 8042, translates, queues, EOI.
void kbd_irq_handler(void);

// -1 when empty, else queued byte ('\n' for Enter, '\b' for Backspace).
int kbd_getc(void);
bool kbd_empty(void);

#endif // NYV_KBD_H
