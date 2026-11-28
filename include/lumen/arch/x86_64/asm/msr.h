#ifndef NYVMSR_H
#define NYVMSR_H

#include <stdint.h>

typedef struct msr_t {
  uint32_t edx;
  uint32_t eax;
} msr_t;

void kset_msr(uint32_t msr, msr_t data);
msr_t kget_msr(uint32_t msr);

#endif // NYVMSR_H
