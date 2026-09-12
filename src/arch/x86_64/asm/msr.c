#include "../../../../include/nyvela/arch/x86_64/asm/msr.h"

void kset_msr(uint32_t msr, msr_t data) {
  __asm__ volatile (
    "wrmsr" : 
            : "d"(data.edx),
              "a"(data.eax),
              "c"(msr) 
  );
}

msr_t kget_msr(uint32_t msr) {
  msr_t res;

  __asm__ volatile (
    "rdmsr" : "=a"(res.eax),
              "=d"(res.edx)
            
            : "c"(msr)
  );

  return res;
}
