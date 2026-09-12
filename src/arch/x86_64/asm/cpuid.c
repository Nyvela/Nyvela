#include "../../../../include/nyvela/arch/x86_64/asm/cpuid.h"

cpuid_result_t cpuid(uint32_t leaf, uint32_t subleaf) {
  cpuid_result_t res;

  __asm__ volatile (
    "cpuid" : "=a"(res.eax),
              "=b"(res.ebx),
              "=c"(res.ecx),
              "=d"(res.edx)

            : "a"(leaf),
              "b"(subleaf)
  );

  return res;
}
