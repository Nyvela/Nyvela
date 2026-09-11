#ifndef NYVVMM_H
#define NYVVMM_H

#include <stdint.h>

bool kvmm_init();

bool kvmmap(uint64_t virt, uint64_t phys, uint64_t flags);
bool kvmunmap(uint64_t virt);

#endif // NYVVMM_H
