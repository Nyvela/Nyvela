#ifndef LUMCNTX_H
#define LUMCNTX_H

#include <stddef.h>
#include <stdint.h>

typedef struct context_t {
  uint64_t rax, rdi, rsi, rcx, rdx, rbx, rbp, rsp;
  uint64_t r8, r9, r10, r11, r12, r13, r14, r15;

  uint64_t rip;
  uint64_t rflags;
  uint64_t cs;
  uint64_t ss;
  uint64_t cr3;
  uint64_t fs_base;
  uint64_t fx_state;
} context_t;

_Static_assert(offsetof(context_t, rax) == 0, "rax offset");
_Static_assert(offsetof(context_t, rdi) == 8, "rdi offset");
_Static_assert(offsetof(context_t, rsi) == 16, "rsi offset");
_Static_assert(offsetof(context_t, rcx) == 24, "rcx offset");
_Static_assert(offsetof(context_t, rdx) == 32, "rdx offset");
_Static_assert(offsetof(context_t, rbx) == 40, "rbx offset");
_Static_assert(offsetof(context_t, rbp) == 48, "rbp offset");
_Static_assert(offsetof(context_t, rsp) == 56, "rsp offset");
_Static_assert(offsetof(context_t, r8) == 64, "r8 offset");
_Static_assert(offsetof(context_t, r9) == 72, "r9 offset");
_Static_assert(offsetof(context_t, r10) == 80, "r10 offset");
_Static_assert(offsetof(context_t, r11) == 88, "r11 offset");
_Static_assert(offsetof(context_t, r12) == 96, "r12 offset");
_Static_assert(offsetof(context_t, r13) == 104, "r13 offset");
_Static_assert(offsetof(context_t, r14) == 112, "r14 offset");
_Static_assert(offsetof(context_t, r15) == 120, "r15 offset");
_Static_assert(offsetof(context_t, rip) == 128, "rip offset");
_Static_assert(offsetof(context_t, rflags) == 136, "rflags offset");
_Static_assert(offsetof(context_t, cs) == 144, "cs offset");
_Static_assert(offsetof(context_t, ss) == 152, "ss offset");
_Static_assert(offsetof(context_t, cr3) == 160, "cr3 offset");

_Static_assert(offsetof(context_t, fs_base) == 168, "fs_base offset");
_Static_assert(offsetof(context_t, fx_state) == 176, "fx_state offset");
_Static_assert(sizeof(context_t) == 184, "context_t has unexpected size");

context_t save_context();
void switch_context(context_t* context);

#endif // LUMCNTX_H
