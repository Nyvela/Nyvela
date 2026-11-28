#include "../../../../include/lumen/drivers/video/console/console.h"
#include "../../../../include/lumen/drivers/video/vga/vga.h"
#include "../../../../include/lumen/arch/x86_64/asm/io.h"

volatile uint8_t *ROW = (volatile uint8_t*)0xC801;
volatile uint8_t *COLUMN = (volatile uint8_t*)0xC800;

/* Mirror console output to COM1 (0x3F8) for headless/serial logging. */
#define COM1 0x3F8
static int serial_ready = 0;

static void serial_init(void) {
  outb(COM1 + 1, 0x00);  /* disable interrupts */
  outb(COM1 + 3, 0x80);  /* DLAB on */
  outb(COM1 + 0, 0x01);  /* divisor lo: 115200 baud */
  outb(COM1 + 1, 0x00);  /* divisor hi */
  outb(COM1 + 3, 0x03);  /* 8N1, DLAB off */
  outb(COM1 + 2, 0xC7);  /* enable + clear FIFO, 14-byte threshold */
  outb(COM1 + 4, 0x0B);  /* RTS/DSR set */
  serial_ready = 1;
}

static void serial_putc(char c) {
  if (!serial_ready) serial_init();
  while ((inb(COM1 + 5) & 0x20) == 0) { }
  outb(COM1, (uint8_t)c);
}

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
    if (col >= VGA_WIDTH) {
      kprintnl();

      row = *ROW;
      col = *COLUMN;
    }
    
    vga_write(col, row, s[i] | (color << 8));
    serial_putc(s[i]);

    ++col;
  }

  *COLUMN = col;
}  

void kprintln(const char *s, const uint8_t color) {
  kprints(s, color);
  kprintnl();
} 

void kprintnl() {
  serial_putc('\r');
  serial_putc('\n');
  *COLUMN = 0;
  *ROW += 1;

  if (*ROW >= VGA_HEIGHT) {
    for (uint16_t row = 0; row < (VGA_HEIGHT - 1) * VGA_WIDTH; row++) {
      VGA_MEM[row] = VGA_MEM[row + VGA_WIDTH];
    }

    vga_fill_row(VGA_HEIGHT - 1, ' ' | (0x07 << 8));

    *ROW = VGA_HEIGHT - 1;
  }
}

void kwrite(const char *buf, uint64_t len, const uint8_t color) {
  if (!buf) return;

  uint8_t row = *ROW;
  uint8_t col = *COLUMN;

  for (uint64_t i = 0; i < len; i++) {
    char c = buf[i];

    if (c == '\n') {
      kprintnl();
      row = *ROW;
      col = *COLUMN;
      continue;
    }

    serial_putc(c);

    if (c == '\b') {
      if (col > 0) {
        col--;
        *COLUMN = col;
        vga_write(col, row, (uint16_t)' ' | (color << 8));
      }
      continue;
    }

    if (col >= VGA_WIDTH) {
      kprintnl();
      row = *ROW;
      col = *COLUMN;
    }

    vga_write(col, row, (uint16_t)c | (color << 8));
    ++col;
    *COLUMN = col;
  }
}

void kclear() {
  for (uint8_t row = 0; row < VGA_HEIGHT; row++) {
    for (uint8_t col = 0; col < VGA_WIDTH; col++) {
      vga_write(col, row, ' ' | (0x07 << 8));
    }
  }

  *ROW = 0;
  *COLUMN = 0;
}

void _print_prefixed_str(const char *s, const char *prefix, const uint16_t colors) {
  kprints(part_lbrack, 0x0F);
  kprints(prefix, (uint8_t)(colors >> 8));
  kprints(part_rbrack, 0x0F);
  kprintln(s, colors & 0x00FF);
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
  _print_prefixed_str(s, part_init, 0x0B00 | color);
}
