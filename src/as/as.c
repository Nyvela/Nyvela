#include "../../include/nyvela/as/as.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/mm/vmm.h"

#include <stddef.h>

addrspace_t** addrspaces = NULL;

uint64_t addrspaces_length = 0;
uint64_t addrspaces_cap = 16;

addrspace_t* kaddrspace_create() {
  if (!addrspaces) {
    addrspaces = kmalloc(sizeof(addrspace_t*) * addrspaces_cap);

    if (!addrspaces) {
      return NULL;
    }
  }

  addrspace_t* as = kmalloc(sizeof(addrspace_t));

  if (!as) {
    return NULL;
  }

  as->cr3 = kvmm_create_user_pml4();
  as->id = addrspaces_length + 1;

  if (!as->cr3) {
    kfree(as);
  }

  if (addrspaces_length >= addrspaces_cap) {
    uint64_t new_cap = addrspaces_cap * 2;

    void *tmp = kmalloc(sizeof(addrspace_t*) * new_cap);

    if (!tmp) {
      kvmm_free_user_pml4(as->cr3);
      kfree(as);

      return NULL;
    }

    addrspaces_cap = new_cap;
    addrspaces = tmp;
  }

  addrspaces[addrspaces_length++] = as;
  return as;
}
