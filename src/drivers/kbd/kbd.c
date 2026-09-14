#include "../../../include/nyvela/drivers/kbd/kbd.h"
#include "../../../include/nyvela/arch/x86_64/asm/io.h"
#include "../../../include/nyvela/arch/x86_64/pic/pic.h"

#define KBD_DATA 0x60
#define KBD_STATUS 0x64
#define KBD_OUT_FULL 0x01

#define KBD_BUF_SIZE 256

static volatile uint8_t kbd_buf[KBD_BUF_SIZE];
static volatile uint16_t kbd_head = 0;
static volatile uint16_t kbd_tail = 0;

static bool kbd_shift_l = false;
static bool kbd_shift_r = false;
static bool kbd_caps = false;
static bool kbd_e0 = false;

static const char kbd_normal[128] = {
  [0x01] = 0x1B,
  [0x02] = '1', [0x03] = '2', [0x04] = '3', [0x05] = '4', [0x06] = '5',
  [0x07] = '6', [0x08] = '7', [0x09] = '8', [0x0A] = '9', [0x0B] = '0',
  [0x0C] = '-', [0x0D] = '=',
  [0x0E] = '\b', [0x0F] = '\t',
  [0x10] = 'q', [0x11] = 'w', [0x12] = 'e', [0x13] = 'r', [0x14] = 't',
  [0x15] = 'y', [0x16] = 'u', [0x17] = 'i', [0x18] = 'o', [0x19] = 'p',
  [0x1A] = '[', [0x1B] = ']',
  [0x1C] = '\n',
  [0x1E] = 'a', [0x1F] = 's', [0x20] = 'd', [0x21] = 'f', [0x22] = 'g',
  [0x23] = 'h', [0x24] = 'j', [0x25] = 'k', [0x26] = 'l',
  [0x27] = ';', [0x28] = '\'', [0x29] = '`',
  [0x2B] = '\\',
  [0x2C] = 'z', [0x2D] = 'x', [0x2E] = 'c', [0x2F] = 'v', [0x30] = 'b',
  [0x31] = 'n', [0x32] = 'm',
  [0x33] = ',', [0x34] = '.', [0x35] = '/',
  [0x39] = ' ',
};

static const char kbd_shifted[128] = {
  [0x01] = 0x1B,
  [0x02] = '!', [0x03] = '@', [0x04] = '#', [0x05] = '$', [0x06] = '%',
  [0x07] = '^', [0x08] = '&', [0x09] = '*', [0x0A] = '(', [0x0B] = ')',
  [0x0C] = '_', [0x0D] = '+',
  [0x0E] = '\b', [0x0F] = '\t',
  [0x10] = 'Q', [0x11] = 'W', [0x12] = 'E', [0x13] = 'R', [0x14] = 'T',
  [0x15] = 'Y', [0x16] = 'U', [0x17] = 'I', [0x18] = 'O', [0x19] = 'P',
  [0x1A] = '{', [0x1B] = '}',
  [0x1C] = '\n',
  [0x1E] = 'A', [0x1F] = 'S', [0x20] = 'D', [0x21] = 'F', [0x22] = 'G',
  [0x23] = 'H', [0x24] = 'J', [0x25] = 'K', [0x26] = 'L',
  [0x27] = ':', [0x28] = '"', [0x29] = '~',
  [0x2B] = '|',
  [0x2C] = 'Z', [0x2D] = 'X', [0x2E] = 'C', [0x2F] = 'V', [0x30] = 'B',
  [0x31] = 'N', [0x32] = 'M',
  [0x33] = '<', [0x34] = '>', [0x35] = '?',
  [0x39] = ' ',
};

static void kbd_push(char c) {
  uint16_t next = (kbd_head + 1) % KBD_BUF_SIZE;

  if (next == kbd_tail) return;

  kbd_buf[kbd_head] = (uint8_t)c;
  kbd_head = next;
}

static void kbd_handle_code(uint8_t code) {
  if (code == 0xE0) {
    kbd_e0 = true;
    return;
  }

  if (kbd_e0) {
    kbd_e0 = false;
    return;
  }

  if (code & 0x80) {
    uint8_t press = code & 0x7F;

    if (press == 0x2A) kbd_shift_l = false;
    if (press == 0x36) kbd_shift_r = false;

    return;
  }

  if (code == 0x2A) {
    kbd_shift_l = true;
    return;
  }

  if (code == 0x36) {
    kbd_shift_r = true;
    return;
  }

  if (code == 0x3A) {
    kbd_caps = !kbd_caps;
    return;
  }

  if (code >= 128) return;

  char base = kbd_normal[code];

  if (!base) return;

  char c;
  bool shift = kbd_shift_l || kbd_shift_r;

  if (base >= 'a' && base <= 'z') {
    c = (shift ^ kbd_caps) ? kbd_shifted[code] : base;
  } else {
    c = shift ? kbd_shifted[code] : base;
    if (!c) c = base;
  }

  kbd_push(c);
}

void kbd_irq_handler(void) {
  for (int i = 0; i < 8; i++) {
    if (!(inb(KBD_STATUS) & KBD_OUT_FULL)) break;

    kbd_handle_code(inb(KBD_DATA));
  }

  kpic_eoi(1);
}

void kbd_init(void) {
  kbd_head = 0;
  kbd_tail = 0;
  kbd_shift_l = false;
  kbd_shift_r = false;
  kbd_caps = false;
  kbd_e0 = false;

  for (int i = 0; i < 32; i++) {
    if (!(inb(KBD_STATUS) & KBD_OUT_FULL)) break;

    (void)inb(KBD_DATA);
  }

  kpic_enable_irq(1);
}

int kbd_getc(void) {
  if (kbd_head == kbd_tail) return -1;

  char c = (char)kbd_buf[kbd_tail];
  kbd_tail = (kbd_tail + 1) % KBD_BUF_SIZE;

  return (unsigned char)c;
}

bool kbd_empty(void) {
  return kbd_head == kbd_tail;
}
