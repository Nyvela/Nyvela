#ifndef NYVVGA_H
#define NYVVGA_H

#include <stdint.h>

extern volatile uint16_t* VGA_MEM;
extern uint8_t VGA_HEIGHT;
extern uint8_t VGA_WIDTH;

void vga_write(uint8_t col, uint8_t row, uint16_t data);
uint16_t vga_read(uint8_t col, uint8_t row);

void vga_fill_row(uint8_t row, uint16_t data);

#endif // NYVIO_H
