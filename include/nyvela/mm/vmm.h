#ifndef NYVVMM_H
#define NYVVMM_H

#include <stdint.h>

#define USER_CODE_VIRT 0x400000ULL
#define USER_STACK_TOP 0x450000ULL
#define USER_STACK_PAGE (USER_STACK_TOP - 0x1000ULL)

#define SHELL_PAGES 4ULL
#define SHELL_MAX (SHELL_PAGES * 4096ULL)

#define PROG_BASE 0x500000ULL
#define PROG_PAGES 16ULL
#define PROG_MAX (PROG_PAGES * 4096ULL)
#define PROG_STACK_TOP 0x600000ULL
#define PROG_STACK_PAGE (PROG_STACK_TOP - 0x1000ULL)

bool kvmm_init();

bool kvmmap(uint64_t virt, uint64_t phys, uint64_t flags);
bool kvmunmap(uint64_t virt);

uint64_t kvmm_create_user_pml4(void);

#endif // NYVVMM_H
