#ifndef LUM_FAULT_H
#define LUM_FAULT_H

#include <stdint.h>

void fault_dump(uint64_t vec, uint64_t err, uint64_t has_err, uint64_t *frame);

#endif // LUM_FAULT_H
