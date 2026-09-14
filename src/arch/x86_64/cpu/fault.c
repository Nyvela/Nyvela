#include "../../../../include/nyvela/arch/x86_64/fault.h"
#include "../../../../include/nyvela/drivers/video/console/console.h"
#include "../../../../include/nyvela/arch/x86_64/asm/cpu.h"

static void put_hex64(uint64_t v) {
  char buf[19];

  buf[0] = '0';
  buf[1] = 'x';

  for (int i = 0; i < 16; i++) {
    uint8_t n = (v >> (60 - i * 4)) & 0xF;
    buf[2 + i] = n < 10 ? (char)('0' + n) : (char)('a' + n - 10);
  }

  buf[18] = '\0';
  kprints(buf, 0x0F);
}

static void put_bytes(const uint8_t *p) {
  for (int i = 0; i < 16; i++) {
    char b[3];
    uint8_t v = p[i];

    b[0] = "0123456789abcdef"[v >> 4];
    b[1] = "0123456789abcdef"[v & 0xF];
    b[2] = '\0';

    kprints(b, 0x0F);
    kprints(" ", 0x0F);
  }

  kprintnl();
}

void fault_dump(uint64_t vec, uint64_t err, uint64_t has_err, uint64_t *frame) {
  uint64_t rip = frame[0];
  uint64_t cs = frame[1];
  uint64_t rflags = frame[2];
  uint64_t rsp;

  if ((cs & 3) == 3) {
    rsp = frame[3];
  } else {
    rsp = (uint64_t)(frame + 3);
  }

  kprintferr("FAULT dump:", 0x0F);

  kprints("vec=", 0x0F);
  put_hex64(vec);
  kprintnl();

  if (has_err) {
    kprints("err=", 0x0F);
    put_hex64(err);
    kprintnl();
  }

  kprints("rip=", 0x0F);
  put_hex64(rip);
  kprintnl();

  kprints("cs=", 0x0F);
  put_hex64(cs);
  kprintnl();

  kprints("rflags=", 0x0F);
  put_hex64(rflags);
  kprintnl();

  kprints("rsp=", 0x0F);
  put_hex64(rsp);
  kprintnl();

  kprints("code: ", 0x0F);
  put_bytes((const uint8_t *)rip);

  hang();
}
