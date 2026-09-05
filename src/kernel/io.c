#include "io.h"

volatile uint8_t* VGA_MEM = (volatile uint8_t*)0xB8000;

volatile uint8_t *ROW = (volatile uint8_t*)0xC701;
volatile uint8_t *COLUMN = (volatile uint8_t*)0xC700;

const char *part_init = "INIT";
const char *part_info = "INFO";
const char *part_succ = "SUCC";
const char *part_fail = "FAIL";
const char *part_ferr = "FERR";
const char *part_lbrack = "[ ";
const char *part_rbrack = " ] ";

void kprints(const char *s, const uint8_t color) {
  uint8_t row = *ROW;
  uint8_t col = *COLUMN;

  for (uint64_t i = 0; s[i]; i++) {
    if (col >= 80) kprintnl();

    VGA_MEM[(row * 80 + col) * 2] = s[i];
    VGA_MEM[(row * 80 + col) * 2 + 1] = color;
    ++col;
  }

  *COLUMN = col;
}  

void kprintln(const char *s, const uint8_t color) {
  kprints(s, color);
  kprintnl();
} 

void kprintnl() {
  *COLUMN = 0;
  *ROW++;

  if (*ROW >= 25) {
    for (uint8_t i = 0; i < 80; i++) {
      VGA_MEM[i] = VGA_MEM[i + 80];
    }

    for (uint8_t i = 0; i < 80; i++) {
      VGA_MEM[24 * 80 + i] = ' ';
    }

    *ROW = 24;
  }
}

void _print_prefixed_str(const char *s, const char *prefix, const uint16_t colors) {
  kprints(part_lbrack, 0x0F);
  kprints(prefix, colors & 0xFF);
  kprints(part_rbrack, 0x0F);
  kprints(s, colors & 0x00FF);
}

void kprintfail(const char *s, const uint8_t color) {
  _print_prefixed_str(s, part_fail, 0x0C00 | color);
}


void kprintinfo(const char *s, const uint8_t color) {
  _print_prefixed_str(s, part_info, 0x0900 | color);
}


void kprintsucc(const char *s, const uint8_t color) {
  _print_prefixed_str(s, part_succ, 0x0A00 | color);
}


void kprintferr(const char *s, const uint8_t color) {
  _print_prefixed_str(s, part_ferr, 0x0400 | color);
}

void kprintinit(const char *s, const uint8_t color) {
  _print_prefixed_str(s, part_ferr, 0x0B00 | color);
}
