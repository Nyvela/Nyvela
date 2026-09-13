#include "../../../../include/nyvela/arch/x86_64/gdt.h"
#include "../../../../include/nyvela/lib/utils.h"

extern uint8_t kernel_stack_top[];

static tss_t tss;

void encode_tss(uint64_t base, uint32_t limit, uint8_t* entry) {
  entry[0] = limit & 0xFF;
  entry[1] = (limit >> 8) & 0xFF;

  entry[2] = base & 0xFF;
  entry[3] = (base >> 8) & 0xFF;
  entry[4] = (base >> 16) & 0xFF;

  entry[5] = 0x89; // Present, DPL0, 64-bit available TSS  
  
  entry[6] = (limit >> 16) & 0x0F;
  entry[7] = (base >> 24) & 0xFF;

  entry[8] = (base >> 32) & 0xFF;
  entry[9] = (base >> 40) & 0xFF;
  entry[10] = (base >> 48) & 0xFF;
  entry[11] = (base >> 56) & 0xFF;

  entry[12] = 0;
  entry[13] = 0;
  entry[14] = 0;
  entry[15] = 0;
}

bool ktss_init() {
  memset(&tss, 0, sizeof(tss));

  tss.rsp0 = (uint64_t)kernel_stack_top;
  tss.iomap_base = sizeof(tss);

  gdtr_t gdtr;

  __asm__ volatile (
      "sgdt %0" : "=m"(gdtr)
  );
  
  encode_tss((uint64_t)&tss, sizeof(tss) - 1, (uint8_t*)(gdtr.base + 0x30));

  __asm__ volatile ("ltr %0" : : "r"(0x30));
  
  return true;
}

void tss_set_rsp0(uint64_t rsp0) {
  tss.rsp0 = rsp0;
}
