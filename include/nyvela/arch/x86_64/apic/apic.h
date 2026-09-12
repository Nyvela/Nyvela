#ifndef NYVAPIC_H
#define NYVAPIC_H

#define IA32_APIC_BASE_MSR 0x1B
#define IA32_APIC_BASE_MSR_MBR 0x100 // Processor is a BSP
#define IA32_APIC_BASE_MSR_ENABLE 0x800

#define LAPIC_SVR 0xF0
#define LAPIC_SVR_ENABLE (1u << 8)

#define LAPIC_VIRT 0xFEE00000ULL

#define LAPIC_EOI 0x0B0

#define LAPIC_TIMER_DIVIDE 0x3E0
#define LAPIC_TIMER_INIT 0x380
#define LAPIC_TIMER_VECTOR 0x20
#define LAPIC_DIVIDE_BY_16 0x03
#define LAPIC_LVT_TIMER 0x320
#define LAPIC_TIMER_PERIODIC (1 << 17)

#include <stdint.h>

bool kenable_lapic();
bool ksetup_lapic_timer();
void klapic_eoi();

#endif // NYVAPIC_H
