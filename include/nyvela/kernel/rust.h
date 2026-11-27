#ifndef NYVELA_KERNEL_RUST_H
#define NYVELA_KERNEL_RUST_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct nyvela_image {
  const uint8_t *bytes;
  size_t length;
} nyvela_image_t;

/* Call during early boot after VGA setup, with exclusive console access
 * and before starting interrupts or the scheduler. */
void nyvela_rust_init(void);
void nyvela_smp_init(void);
nyvela_image_t nyvela_fat_image(void);
bool nyvela_map_exec_stack(uint64_t cr3, uint64_t initial_page);

#endif
