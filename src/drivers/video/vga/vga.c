#include "../../../../include/nyvela/drivers/video/vga/vga.h"

volatile uint16_t* VGA_MEM = (volatile uint16_t*)0xB8000;

uint8_t VGA_WIDTH = 80;
uint8_t VGA_HEIGHT = 25;

void vga_write(uint8_t col, uint8_t row, uint16_t data) {
  VGA_MEM[row * VGA_WIDTH + col] = data;
}

uint16_t vga_read(uint8_t col, uint8_t row) {
  return VGA_MEM[(row * VGA_WIDTH + col)];
}

void vga_copy(uint8_t dest_col, uint8_t dest_row, uint8_t src_col, uint8_t src_row) {
  for (uint8_t i = 0; i < VGA_WIDTH; i++) {
    VGA_MEM[dest_row * VGA_WIDTH + dest_col + i] = VGA_MEM[src_row * VGA_WIDTH + src_col + i];
  }
}

void vga_fill_row(uint8_t row, uint16_t data) {
  for (uint8_t col = 0; col < VGA_WIDTH; col++) {
    VGA_MEM[row * VGA_WIDTH + col] = data;
  }
}
