#include <nyvela/arch/x86_64/asm/cpu.h>
#include <nyvela/drivers/video/console/console.h>

_Noreturn void nyvela_rust_panic(void) {
  cli();
  kprintferr("Rust kernel panic.", 0x0F);

  for (;;) {
    hlt();
  }
}
