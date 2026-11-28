#include "../../../../include/lumen/arch/x86_64/fault.h"
#include "../../../../include/lumen/drivers/video/console/console.h"
#include "../../../../include/lumen/arch/x86_64/asm/cpu.h"

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

  kprints("cr2=", 0x0F);
  put_hex64(read_cr2());
  kprintnl();

  kprints("cr3=", 0x0F);
  put_hex64(read_cr3() & ~0xFFFULL);
  kprintnl();

  hang();
}
