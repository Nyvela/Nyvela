#ifndef NYVCPU_H
#define NYVCPU_H

#include <stdint.h>

// Central x86-64 CPU abstractions. All raw inline asm for CPU
// control lives here so higher-level kernel code doesn't need inline asm.

static inline void cpu_cli(void) {
  __asm__ volatile ("cli" ::: "memory");
}

static inline void cpu_sti(void) {
  __asm__ volatile ("sti" ::: "memory");
}

static inline void cpu_hlt(void) {
  __asm__ volatile ("hlt" ::: "memory");
}

static inline void cpu_pause(void) {
  __asm__ volatile ("pause" ::: "memory");
}

// Single-shot cli + hlt. Preserves the historical "panic halt" semantics
// used across kernel/mm (executes once, does not loop).
static inline void cpu_halt(void) {
  __asm__ volatile ("cli\n\thlt" ::: "memory");
}

// Unrecoverable halt: disable interrupts and halt forever.
static inline void cpu_halt_forever(void) {
  for (;;) {
    cpu_halt();
  }
}

// Idle step: caller is expected to have interrupts enabled (see cpu_sti)
// and to call this in a loop.
static inline void cpu_idle(void) {
  cpu_hlt();
}

static inline void cpu_relax(void) {
  cpu_pause();
}

static inline void cpu_compiler_barrier(void) {
  __asm__ volatile ("" ::: "memory");
}

static inline uint64_t cpu_read_cr0(void) {
  uint64_t val;
  __asm__ volatile ("mov %%cr0, %0" : "=r"(val));
  return val;
}

static inline void cpu_write_cr0(uint64_t val) {
  __asm__ volatile ("mov %0, %%cr0" :: "r"(val) : "memory");
}

static inline uint64_t cpu_read_cr2(void) {
  uint64_t val;
  __asm__ volatile ("mov %%cr2, %0" : "=r"(val));
  return val;
}

static inline uint64_t cpu_read_cr3(void) {
  uint64_t val;
  __asm__ volatile ("mov %%cr3, %0" : "=r"(val));
  return val;
}

static inline void cpu_write_cr3(uint64_t val) {
  __asm__ volatile ("mov %0, %%cr3" :: "r"(val) : "memory");
}

static inline uint64_t cpu_read_cr4(void) {
  uint64_t val;
  __asm__ volatile ("mov %%cr4, %0" : "=r"(val));
  return val;
}

static inline void cpu_write_cr4(uint64_t val) {
  __asm__ volatile ("mov %0, %%cr4" :: "r"(val) : "memory");
}

static inline void cpu_invlpg(uint64_t addr) {
  __asm__ volatile ("invlpg (%0)" :: "r"(addr) : "memory");
}

// Flush the whole TLB by reloading CR3 (preserves previous behaviour in vmm).
static inline void cpu_flush_tlb(void) {
  cpu_write_cr3(cpu_read_cr3());
}

static inline uint64_t cpu_rdtsc(void) {
  uint32_t lo, hi;
  __asm__ volatile ("rdtsc" : "=a"(lo), "=d"(hi));
  return ((uint64_t)hi << 32) | lo;
}

// Raw iretq with no stack fixup. Only valid when the stack already holds
// ss/rsp/rflags/cs/rip (e.g. hand-written context-switch paths).
static inline void cpu_iretq(void) {
  __asm__ volatile ("iretq" ::: "memory");
}

#endif // NYVCPU_H
