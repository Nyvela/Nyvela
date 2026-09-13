#ifndef NYVDESC_H
#define NYVDESC_H

// Central descriptor-table abstractions (IDT/GDT/LDT/TR).
// Takes `const void *` to a 10-byte limit:base pointer so callers can pass
// `&idtr` / `&gdtr` structs without this header depending on higher-level
// idt/gdt headers. All raw inline asm for these lives here.

static inline void desc_lidt(const void *idtr) {
  __asm__ volatile ("lidt %0" :: "m"(*(const unsigned char (*)[10])idtr));
}

static inline void desc_lgdt(const void *gdtr) {
  __asm__ volatile ("lgdt %0" :: "m"(*(const unsigned char (*)[10])gdtr));
}

static inline void desc_sidt(void *idtr) {
  __asm__ volatile ("sidt %0" : "=m"(*(unsigned char (*)[10])idtr) :: "memory");
}

static inline void desc_sgdt(void *gdtr) {
  __asm__ volatile ("sgdt %0" : "=m"(*(unsigned char (*)[10])gdtr) :: "memory");
}

static inline void desc_ltr(unsigned short selector) {
  __asm__ volatile ("ltr %0" :: "r"(selector) : "memory");
}

static inline unsigned short desc_str(void) {
  unsigned short selector;
  __asm__ volatile ("str %0" : "=r"(selector));
  return selector;
}

#endif // NYVDESC_H
