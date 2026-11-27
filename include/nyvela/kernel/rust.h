#ifndef NYVELA_KERNEL_RUST_H
#define NYVELA_KERNEL_RUST_H

/* Call during early boot after VGA setup, with exclusive console access
 * and before starting interrupts or the scheduler. */
void nyvela_rust_init(void);

#endif
