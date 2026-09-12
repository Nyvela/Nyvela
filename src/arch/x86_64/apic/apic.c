#include "../../../../include/nyvela/arch/x86_64/apic/apic.h"
#include "../../../../include/nyvela/arch/x86_64/asm/msr.h"
#include "../../../../include/nyvela/mm/vmm.h"
#include "../../../../include/nyvela/arch/x86_64/asm/out.h"

static uintptr_t lapic_base;

void kset_apic_base(uintptr_t apic) {
  msr_t msr = (msr_t){ .edx = 0, .eax = (apic & 0xFFFFF000ULL) | IA32_APIC_BASE_MSR_ENABLE};
  kset_msr(IA32_APIC_BASE_MSR, msr);
}

uintptr_t kget_apic_base() {
  msr_t msr = kget_msr(IA32_APIC_BASE_MSR);

  return (msr.eax & 0xFFFFF000ULL);
}

uint32_t klapic_read(uint32_t offset) {
  return *(volatile uint32_t*)(lapic_base + offset);
}

void klapic_write(uint32_t offset, uint32_t value) {
  *(volatile uint32_t*)(lapic_base + offset) = value;
}

bool kenable_lapic() {
  uint64_t lapic_phys = kget_apic_base();

  kvmmap(
    LAPIC_VIRT, lapic_phys, 0x03
  );

  lapic_base = LAPIC_VIRT;

  uint32_t svr = klapic_read(LAPIC_SVR);

  klapic_write(LAPIC_SVR, svr | LAPIC_SVR_ENABLE);

  outb(0x21, 0xFF);
  outb(0xA1, 0xFF);

  return (klapic_read(LAPIC_SVR) & LAPIC_SVR_ENABLE) != 0;
}

bool ksetup_lapic_timer() {
  klapic_write(LAPIC_TIMER_DIVIDE, LAPIC_DIVIDE_BY_16);
  klapic_write(LAPIC_LVT_TIMER, LAPIC_TIMER_VECTOR | LAPIC_TIMER_PERIODIC);
  
  klapic_write(LAPIC_TIMER_INIT, 1000000);

  return true;
}

void klapic_eoi() {
  klapic_write(LAPIC_EOI, 0);
}
