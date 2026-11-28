#include <lumen/arch/x86_64/asm/cpu.h>
#include <lumen/drivers/video/console/console.h>

_Noreturn void lumen_rust_panic(void) {
  cli();
  kprintferr("Rust kernel panic.", 0x0F);

  for (;;) {
    hlt();
  }
}
