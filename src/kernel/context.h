#ifndef NYVCNTX_H
#define NYVCNTX_H

typedef struct context_t {
  uint64_t rax, rdi, rsi, rcx, rdx, rbx, rbp, rsp,
           r8, r9, r10, r11, r12, r13, r14, r15,
           rip, rflags;

  uint64_t cr3;
} context_t;

_Static_assert(sizeof(context_t) == 152, "context_t must be 152 bytes");

context_t save_context();
void load_context(context_t* context);

#endif // NYVCNTX_H
