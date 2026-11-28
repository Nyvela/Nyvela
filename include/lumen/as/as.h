#ifndef NYVAS_H
#define NYVAS_H

#include <stdint.h>

typedef struct addrspace {
  uint64_t cr3;
  uint64_t id;
} addrspace_t;

extern addrspace_t** addrspaces;
extern uint64_t addrspaces_length, addrspaces_size;

addrspace_t* kaddrspace_create();

#endif // NYVAS_H
