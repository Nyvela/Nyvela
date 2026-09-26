#include "../../include/nyvela/mm/vmm.h"
#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/drivers/video/console/console.h"
#include "../../include/nyvela/arch/x86_64/asm/cpu.h"
#include "../../include/nyvela/lib/utils.h"

uint64_t kernel_cr3 = 0;

bool kvmm_init() {
  uint64_t cr3 = read_cr3();

  kernel_cr3 = cr3 & ~0xFFFULL;

  uint64_t pml4_phys = cr3 & ~0xFFFULL;
  uint64_t *pml4 = (uint64_t*)pml4_phys;

  uint64_t pdpt_phys = pml4[0] & ~0xFFFULL;
  uint64_t *pdpt = (uint64_t*)pdpt_phys;
  
  uint64_t pd_phys = pdpt[0] & ~0xFFFULL;
  uint64_t *pd = (uint64_t*)pd_phys;

  for (uint64_t i = 0; i < 512; i++) {   
    uint64_t pt_phys = (uint64_t)kpalloc();

    if (!pt_phys) {
      kprintferr("Failed to allocate memory for PT entry.", 0x0F);
      return false;
    }

    uint64_t* pt = (uint64_t*)pt_phys;

    for (uint64_t j = 0; j < 512; j++) {
      uint64_t phys = (i * 0x200000) + (j * 0x1000);
      pt[j] = phys | 0x03;
    }

    pd[i] = pt_phys | 0x03;
  }

  write_cr3(cr3);

  return true;
}

uint64_t* kget_pte_addr_at(uint64_t cr3, uint64_t virt) {
  uint64_t flags = irq_save();

  uint64_t pml4_index = (virt >> 39) & 0x1FFULL;
  uint64_t pdpt_index = (virt >> 30) & 0x1FFULL;
  uint64_t pd_index = (virt >> 21) & 0x1FFULL;
  uint64_t pt_index = (virt >> 12) & 0x1FFULL;
  
  uint64_t pml4_phys = cr3 & ~0xFFFULL;
  uint64_t *pml4 = (uint64_t*)pml4_phys;

  uint64_t pml4e = pml4[pml4_index];
  
  uint64_t pdpt_phys;
  uint64_t *pdpt;

  if (!(pml4e & 1)) {
    write_flags(flags);
    return 0;
  } else {
    pdpt_phys = pml4e & ~0xFFFULL;
    pdpt = (uint64_t*)pdpt_phys;
  }
  
  uint64_t pdpte = pdpt[pdpt_index];
  
  uint64_t pd_phys;
  uint64_t *pd;

  if (!(pdpte & 1)) {
    write_flags(flags);
    return 0;
  } else {
    pd_phys = pdpte & ~0xFFFULL;
    pd = (uint64_t*)pd_phys;
  }
  
  uint64_t pde = pd[pd_index];

  uint64_t pt_phys;
  
  if (!(pde & 1)) {
    write_flags(flags);
    return 0;
  } else {
    pt_phys = pde & ~0xFFFULL;
  }
  
  write_flags(flags);
  return &((uint64_t*)(pt_phys & ~0xFFFULL))[pt_index];
}

uint64_t* kget_pte_addr(uint64_t virt) {
  return kget_pte_addr_at(read_cr3(), virt);
}

uint64_t kget_phys_page_addr_at(uint64_t cr3, uint64_t virt) {
  uint64_t *pte = kget_pte_addr_at(cr3, virt);
  return pte ? (*pte & ~0xFFFULL) : 0;
}

uint64_t kget_phys_page_addr(uint64_t virt) {
  return kget_phys_page_addr_at(read_cr3(), virt);
}

bool kvmunmap_and_free_at(uint64_t cr3, uint64_t virt) {
  uint64_t flags = irq_save();
  uint64_t *pte = kget_pte_addr_at(cr3, virt);

  if (!pte) {
    write_flags(flags);
    return false;
  }

  uint64_t phys = *pte & ~0xFFFULL;

  kpfree((void*)phys);
  *pte = 0;

  if ((cr3 & ~0xFFFULL) == (read_cr3() & ~0xFFFULL)) invlpg(virt);
  write_flags(flags);
  return true;
}

bool kvmunmap_and_free(uint64_t virt) {
  return kvmunmap_and_free_at(read_cr3(), virt);
}

bool kvmunmap_at(uint64_t cr3, uint64_t virt) {
  uint64_t flags = irq_save();
  uint64_t *pte = kget_pte_addr_at(cr3, virt);

  if (!pte || !(*pte & 1)) {
    write_flags(flags);
    return false;
  }

  *pte = 0;

  if ((cr3 & ~0xFFFULL) == (read_cr3() & ~0xFFFULL)) invlpg(virt);
  write_flags(flags);
  return true;
}

bool kvmunmap(uint64_t virt) { 
  return kvmunmap_at(read_cr3(), virt);
}

bool kvmmap_at(uint64_t cr3, uint64_t virt, uint64_t phys, uint64_t flags) {
  uint64_t _flags = irq_save();

  uint64_t pml4_index = (virt >> 39) & 0x1FFULL;
  uint64_t pdpt_index = (virt >> 30) & 0x1FFULL;
  uint64_t pd_index = (virt >> 21) & 0x1FFULL;
  uint64_t pt_index = (virt >> 12) & 0x1FFULL;
  
  uint64_t pml4_phys = cr3 & ~0xFFFULL;
  uint64_t *pml4 = (uint64_t*)pml4_phys;

  uint64_t pml4e = pml4[pml4_index];
  
  uint64_t pdpt_phys;
  uint64_t *pdpt;

  uint64_t table_flags = 0x03;

  if (flags & 0x04) {
    table_flags |= 0x04;
  }

  if (!(pml4e & 1)) {
    pdpt_phys = (uint64_t)kpalloc();

    if (!pdpt_phys) {
      kprintferr("Failed to allocate PDPT entry.", 0x0F);
      hang();
    }
     
    pdpt = (uint64_t*)pdpt_phys;

    for (uint64_t i = 0; i < 512; i++) {
      pdpt[i] = 0;
    }

    pml4[pml4_index] = pdpt_phys | table_flags;
  } else {
    pdpt_phys = pml4e & ~0xFFFULL;
    pdpt = (uint64_t*)pdpt_phys;
  }
  
  uint64_t pdpte = pdpt[pdpt_index];
  
  uint64_t pd_phys;
  uint64_t *pd;

  if (!(pdpte & 1)) {
    pd_phys = (uint64_t)kpalloc();

    if (!pd_phys) {
      kprintferr("Failed to allocate PD entry.", 0x0F);
      hang();
    }
     
    pd = (uint64_t*)pd_phys;

    for (uint64_t i = 0; i < 512; i++) {
      pd[i] = 0;
    }

    pdpt[pdpt_index] = pd_phys | table_flags;
  } else {
    pd_phys = pdpte & ~0xFFFULL;
    pd = (uint64_t*)pd_phys;
  }
  
  uint64_t pde = pd[pd_index];

  uint64_t pt_phys;
  uint64_t *pt;
  
  if (!(pde & 1)) {
    pt_phys = (uint64_t)kpalloc();

    if (!pt_phys) {
      kprintferr("Failed to allocate PD entry.", 0x0F);
      hang();
    }
     
    pt = (uint64_t*)pt_phys;

    for (uint64_t i = 0; i < 512; i++) {
      pt[i] = 0;
    }

    pd[pd_index] = pt_phys | table_flags;
  } else {
    pt_phys = pde & ~0xFFFULL;
    pt = (uint64_t*)pt_phys;
  }

  if (flags & 0x04) {
    pml4[pml4_index] |= 0x04;
    pdpt[pdpt_index] |= 0x04;
    pd[pd_index] |= 0x04;
  }

  pt[pt_index] = (phys & ~0xFFFULL) | flags;
  if ((cr3 & ~0xFFFULL) == (read_cr3() & ~0xFFFULL)) invlpg(virt);
  
  write_flags(_flags);
  return true;
}

bool kvmmap(uint64_t virt, uint64_t phys, uint64_t flags) { 
  uint64_t _flags = irq_save();

  uint64_t cr3 = read_cr3();
  kvmmap_at(cr3, virt, phys, flags);
  write_cr3(cr3); // flush
  
  write_flags(_flags);
  return true;
}

uint64_t kvmm_create_user_pml4(void) {
  uint64_t flags = irq_save();
  uint64_t new_pml4_phys = (uint64_t)kpalloc();

  if (!new_pml4_phys) {
    write_flags(flags);
    return 0;
  }

  uint64_t old_cr3 = read_cr3();
  uint64_t *old_pml4 = (uint64_t*)(old_cr3 & ~0xFFFULL);
  uint64_t *new_pml4 = (uint64_t*)new_pml4_phys;

  memset(new_pml4, 0, 0x1000);

  for (uint64_t i = 1; i < 512; i++) {
    new_pml4[i] = old_pml4[i];
  }
  
  uint64_t new_pdpt_phys = (uint64_t)kpalloc();
  
  if (!new_pdpt_phys) {
    kpfree((void*)new_pml4_phys);
    write_flags(flags);
    return 0;
  }

  uint64_t *old_pdpt = (uint64_t*)(old_pml4[0] & ~0xFFFULL);
  uint64_t *new_pdpt = (uint64_t*)new_pdpt_phys;

  memcpy(new_pdpt, old_pdpt, 0x1000);

  uint64_t new_pd_phys = (uint64_t)kpalloc();
  
  if (!new_pd_phys) {
    kpfree((void*)new_pml4_phys);
    kpfree((void*)new_pdpt_phys);
    write_flags(flags);
    return 0;
  }
  
  uint64_t* old_pd = (uint64_t*)(old_pdpt[0] & ~0xFFFULL);
  uint64_t* new_pd = (uint64_t*)new_pd_phys;

  memcpy(new_pd, old_pd, 0x1000);
  
  uint64_t *old_pt2 = NULL;
  uint64_t *old_pt3 = NULL;
  
  uint64_t new_pt2_phys = 0;
  uint64_t new_pt3_phys = 0;
  
  uint64_t* new_pt2 = NULL;
  uint64_t* new_pt3 = NULL;

  if (old_pd[2] & 1) {
    old_pt2 = (uint64_t*)(old_pd[2] & ~0xFFFULL);
    
    new_pt2_phys = (uint64_t)kpalloc();

    if (!new_pt2_phys) {
      kpfree((void*)new_pml4_phys);
      kpfree((void*)new_pdpt_phys);
      kpfree((void*)new_pd_phys);
      write_flags(flags);
      return 0;
    }
    
    memset((void*)new_pt2_phys, 0, 0x1000);
   
    new_pt2 = (uint64_t*)new_pt2_phys; 
    
    for (uint64_t i = 0; i < 512; i++) {
      if (!(old_pt2[i] & 0x01))
        continue;
      
      uint64_t virt = USER_CODE_VIRT + i * 4096;
      bool shared = is_shared_user_page(virt);
      
      if (shared) {
        new_pt2[i] = old_pt2[i];
      } else if (old_pt2[i] & 0x04) {
        uint64_t np = (uint64_t)kpalloc();

        if (!np) {
          for (uint64_t j = 0; j < i; j++) {
            if (new_pt2[j] & 0x01 && new_pt2[j] & 0x04) 
              kpfree((void*)(new_pt2[j] & ~0xFFFULL));
          }

          kpfree((void*)new_pml4_phys);
          kpfree((void*)new_pdpt_phys);
          kpfree((void*)new_pd_phys);
          kpfree((void*)new_pt2_phys);
          
          write_flags(flags);
          return 0;
        }

        memcpy((void*)np, (void*)(old_pt2[i] & ~0xFFFULL), 0x1000);
        new_pt2[i] = (np & ~0xFFFULL) | (old_pt2[i] & 0xFFFULL);
      } else {
        new_pt2[i] = old_pt2[i];
      }
    }

    new_pd[2] = (new_pt2_phys & ~0xFFF) | 0x03 | (new_pd[2] & 0x04);
  } else {
    new_pd[2] = 0;
  }

  if (old_pd[3] & 1) {
    old_pt3 = (uint64_t*)(old_pd[3] & ~0xFFFULL);
  
    new_pt3_phys = (uint64_t)kpalloc(); 

    if (!new_pt3_phys) {
      for (uint64_t i = 0; i < 512; i++) {
        if (new_pt2[i] & 1) {
          kpfree((void*)(new_pt2[i] & ~0xFFFULL));
        }
      }

      kpfree((void*)new_pml4_phys);
      kpfree((void*)new_pdpt_phys);
      kpfree((void*)new_pd_phys);
      kpfree((void*)new_pt2_phys);
      write_flags(flags);
      return 0;
    }

    memset((void*)new_pt3_phys, 0, 0x1000); 
    
    new_pt3 = (uint64_t*)new_pt3_phys;

    for (uint64_t i = 0; i < 512; i++) {
      if (!(old_pt3[i] & 0x01))
        continue;
      
      uint64_t virt = PROG_STACK_TOP + i * 4096;
      bool shared = is_shared_user_page(virt);
      
      if (shared) {
        new_pt3[i] = old_pt3[i];
      } else if (old_pt3[i] & 0x04) {
        uint64_t np = (uint64_t)kpalloc();

        if (!np) {
          for (uint64_t j = 0; j < i; j++) {
            if (new_pt3[j] & 0x01 && new_pt3[j] & 0x04) 
              kpfree((void*)(new_pt3[j] & ~0xFFFULL));
          }

          for (uint64_t j = 0; j < 512; j++) {
            if (new_pt2[j] & 0x01 && new_pt2[j] & 0x04) 
              kpfree((void*)(new_pt2[j] & ~0xFFFULL));
          }
          
          kpfree((void*)new_pml4_phys);
          kpfree((void*)new_pdpt_phys);
          kpfree((void*)new_pd_phys);
          kpfree((void*)new_pt2_phys);
          kpfree((void*)new_pt3_phys);
          
          write_flags(flags);
          return 0;
        }
          
        memcpy((void*)np, (void*)(old_pt3[i] & ~0xFFFULL), 0x1000);
        new_pt3[i] = (np & ~0xFFFULL) | (old_pt3[i] & 0xFFFULL);
      } else {
        new_pt3[i] = old_pt3[i];
      }
    }

    new_pd[3] = (new_pt3_phys & ~0xFFF) | 0x03 | (new_pd[3] & 0x04);
  } else {
    new_pd[3] = 0;
  }
 
  new_pml4[0] = (new_pdpt_phys & ~0xFFF) | 0x03 | (old_pml4[0] & 0x04);
  new_pdpt[0] = (new_pd_phys & ~0xFFF) | 0x03 | (old_pdpt[0] & 0x04);
  
  write_flags(flags);
  return new_pml4_phys;
}

void kvmm_free_user_pml4(uint64_t pml4_phys) {
  uint64_t flags = irq_save();

  if (!pml4_phys) { 
    write_flags(flags);
    return;
  }

  if ((pml4_phys & ~0xFFFULL) == (read_cr3() & ~0xFFFULL)) {
    kprintferr("kvmm_free_user_pml4: refusing to free active cr3 (leaked).", 0x0F);
    write_flags(flags);
    return;
  }
  
  uint64_t pml4e = *(uint64_t*)pml4_phys;

  if (!(pml4e & 0x01)) {
    kpfree((void*)pml4_phys);
    write_flags(flags);
    return;
  }

  uint64_t *pdpt = (void*)(pml4e & ~0xFFFULL);
  uint64_t pdpte = pdpt[0];

  if (!(pdpte & 0x01)) {
    kpfree(pdpt);
    kpfree((void*)pml4_phys);
    write_flags(flags);
    return;
  }

  uint64_t *pd = (void*)(pdpt[0] & ~0xFFFULL);

  if (pd[2] & 0x01) {
    uint64_t *pt2 = (void*)(pd[2] & ~0xFFFULL);

    for (uint64_t i = 0; i < 512; i++) {
      if (!(pt2[i] & 0x01 && pt2[i] & 0x04))
        continue;
      
      uint64_t virt = USER_CODE_VIRT + i * 4096;

      if (is_shared_user_page(virt))
        continue;

      kpfree((void*)(pt2[i] & ~0xFFFULL));
    }

    kpfree(pt2);
  }

  if (pd[3] & 0x01) {
    uint64_t *pt3 = (void*)(pd[3] & ~0xFFFULL);

    for (uint64_t i = 0; i < 512; i++) {
      if (!(pt3[i] & 0x01 && pt3[i] & 0x04))
        continue;
      
      uint64_t virt = PROG_STACK_TOP + i * 4096;

      if (is_shared_user_page(virt))
        continue;

      kpfree((void*)(pt3[i] & ~0xFFFULL));
    }

    kpfree(pt3);
  }
  
  kpfree(pd);
  kpfree(pdpt);
  kpfree((void*)pml4_phys);
  write_flags(flags);
}

bool is_shared_user_page(uint64_t virt) {
  if (virt >= USER_CODE_VIRT && virt < USER_CODE_VIRT + SHELL_MAX)
    return true;

  if (virt >= USER_STACK_PAGE && virt < USER_STACK_TOP)
    return true;

  return false;
}
