#ifndef LUMEN_KERNEL_RUST_H
#define LUMEN_KERNEL_RUST_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct lumen_image {
  const uint8_t *bytes;
  size_t length;
} lumen_image_t;

/* Call during early boot after VGA setup, with exclusive console access
 * and before starting interrupts or the scheduler. */
void lumen_rust_init(void);
void lumen_smp_init(void);
lumen_image_t lumen_fat_image(void);
bool lumen_map_exec_stack(uint64_t cr3, uint64_t initial_page);

#endif
