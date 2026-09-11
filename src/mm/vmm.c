#include "../../include/nyvela/mm/vmm.h"
#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/drivers/video/console/console.h"

bool kvmm_init() {
  uint64_t cr3;
  __asm__ volatile ("mov %%cr3, %0" : "=r"(cr3));

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

  __asm__ volatile ("mov %0, %%cr3" :: "r"(cr3) : "memory");

  return true;
}

bool kvmunmap(uint64_t virt) { 
  uint64_t cr3;
  __asm__ volatile ("mov %%cr3, %0" : "=r"(cr3));
  
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
    return false;
  } else {
    pdpt_phys = pml4e & ~0xFFFULL;
    pdpt = (uint64_t*)pdpt_phys;
  }
  
  uint64_t pdpte = pdpt[pdpt_index];
  
  uint64_t pd_phys;
  uint64_t *pd;

  if (!(pdpte & 1)) {
    return false;
  } else {
    pd_phys = pdpte & ~0xFFFULL;
    pd = (uint64_t*)pd_phys;
  }
  
  uint64_t pde = pd[pd_index];

  uint64_t pt_phys;
  uint64_t *pt;
  
  if (!(pde & 1)) {
    return false;
  } else {
    pt_phys = pde & ~0xFFFULL;
    pt = (uint64_t*)pt_phys;
  }

  pt[pt_index] = 0;
  __asm__ volatile ("mov %0, %%cr3" :: "r"(cr3) : "memory");

  return true;
}

bool kvmmap(uint64_t virt, uint64_t phys, uint64_t flags) { 
  uint64_t cr3;
  __asm__ volatile ("mov %%cr3, %0" : "=r"(cr3));
  
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
    pdpt_phys = (uint64_t)kpalloc();

    if (!pdpt_phys) {
      kprintferr("Failed to allocate PDPT entry.", 0x0F);
      __asm__ volatile ("cli\nhlt");
    }
     
    pdpt = (uint64_t*)pdpt_phys;

    for (uint64_t i = 0; i < 512; i++) {
      pdpt[i] = 0;
    }

    pml4[pml4_index] = pdpt_phys | 0x03;
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
      __asm__ volatile ("cli\nhlt");
    }
     
    pd = (uint64_t*)pd_phys;

    for (uint64_t i = 0; i < 512; i++) {
      pd[i] = 0;
    }

    pdpt[pdpt_index] = pd_phys | 0x03;
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
      __asm__ volatile ("cli\nhlt");
    }
     
    pt = (uint64_t*)pt_phys;

    for (uint64_t i = 0; i < 512; i++) {
      pt[i] = 0;
    }

    pd[pd_index] = pt_phys | 0x03;
  } else {
    pt_phys = pde & ~0xFFFULL;
    pt = (uint64_t*)pt_phys;
  }

  pt[pt_index] = (phys & ~0xFFFULL) | flags;
  __asm__ volatile ("mov %0, %%cr3" :: "r"(cr3) : "memory");

  return true;
}
