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

typedef struct vm_area {
  uint64_t start, end;
  uint64_t flags;
  struct vm_area *next;
} vm_area_t;

bool kvmm_init();

extern uint64_t kernel_cr3;

bool kvmmap_at(uint64_t cr3, uint64_t virt, uint64_t phys, uint64_t flags);
bool kvmmap(uint64_t virt, uint64_t phys, uint64_t flags);

bool kvmunmap(uint64_t virt);
bool kvmunmap_at(uint64_t cr3, uint64_t virt);

bool kvmunmap_and_free(uint64_t virt);
bool kvmunmap_and_free_at(uint64_t cr3, uint64_t virt);

uint64_t* kget_pte_addr(uint64_t virt);
uint64_t* kget_pte_addr_at(uint64_t cr3, uint64_t virt);

uint64_t kget_phys_page_addr(uint64_t virt);
uint64_t kget_phys_page_addr_at(uint64_t cr3, uint64_t virt);

uint64_t kvmm_create_user_pml4(void);
void kvmm_free_user_pml4(uint64_t pml4);

bool is_shared_user_page(uint64_t virt);

#endif // NYVVMM_H
