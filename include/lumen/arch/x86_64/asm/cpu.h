#ifndef NYVCPU_H
#define NYVCPU_H

#include <stdint.h>

static inline uint64_t read_flags() {
  uint64_t flags;
  __asm__ volatile ("pushfq; pop %0" : "=r"(flags) :: "memory");
  return flags;
}

static inline void write_flags(uint64_t flags) {
  __asm__ volatile ("push %0; popfq" :: "r"(flags) : "memory", "cc");
}

static inline void cli(void) {
  __asm__ volatile ("cli" ::: "memory");
}

static inline void sti(void) {
  __asm__ volatile ("sti" ::: "memory");
}

static inline uint64_t irq_save() {
  uint64_t flags = read_flags();
  cli();
  return flags;
}

static inline void hlt(void) {
  __asm__ volatile ("hlt" ::: "memory");
}

static inline void pause(void) {
  __asm__ volatile ("pause" ::: "memory");
}

static inline void hang(void) {
  __asm__ volatile ("cli\n\thlt" ::: "memory");
}

static inline void __compiler_barrier(void) {
  __asm__ volatile ("" ::: "memory");
}

static inline uint64_t read_cr0(void) {
  uint64_t val;
  __asm__ volatile ("mov %%cr0, %0" : "=r"(val));
  return val;
}

static inline void write_cr0(uint64_t val) {
  __asm__ volatile ("mov %0, %%cr0" :: "r"(val) : "memory");
}

static inline uint64_t read_cr2(void) {
  uint64_t val;
  __asm__ volatile ("mov %%cr2, %0" : "=r"(val));
  return val;
}

static inline uint64_t read_cr3(void) {
  uint64_t val;
  __asm__ volatile ("mov %%cr3, %0" : "=r"(val));
  return val;
}

static inline void write_cr3(uint64_t val) {
  __asm__ volatile ("mov %0, %%cr3" :: "r"(val) : "memory");
}

static inline uint64_t read_cr4(void) {
  uint64_t val;
  __asm__ volatile ("mov %%cr4, %0" : "=r"(val));
  return val;
}

static inline void write_cr4(uint64_t val) {
  __asm__ volatile ("mov %0, %%cr4" :: "r"(val) : "memory");
}

static inline void invlpg(uint64_t addr) {
  __asm__ volatile ("invlpg (%0)" :: "r"(addr) : "memory");
}

static inline void flush_tlb(void) {
  write_cr3(read_cr3());
}

static inline uint64_t rdtsc(void) {
  uint32_t lo, hi;
  __asm__ volatile ("rdtsc" : "=a"(lo), "=d"(hi));
  return ((uint64_t)hi << 32) | lo;
}

static inline void iretq(void) {
  __asm__ volatile ("iretq" ::: "memory");
}

#endif // NYVCPU_H
