#ifndef NYVCPUID_H
#define NYVCPUID_H

#include <stdint.h>

typedef struct cpuid_result_t {
  uint32_t eax;
  uint32_t ebx;
  uint32_t ecx;
  uint32_t edx;
} cpuid_result_t;

cpuid_result_t cpuid(uint32_t leaf, uint32_t subleaf);

#endif // NYVCPUID_H
