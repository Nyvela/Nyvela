#ifndef NYV_SYSCALL_H
#define NYV_SYSCALL_H

#include <stdint.h>
#include <stdbool.h>

// Minimal syscall ABI via "int $0x80" (DPL3 gate at vector 0x80).
// rax = number, args = rdi, rsi, rdx, r10, r8, r9. Return in rax.
// Errors are VFS_ERR_* (negative) cast to u64.

#define SYS_EXIT      0ULL
#define SYS_WRITE     1ULL
#define SYS_YIELD     2ULL
#define SYS_READ      3ULL

#define SYS_FS_CREATE 10ULL
#define SYS_FS_WRITE  11ULL
#define SYS_FS_READ   12ULL
#define SYS_FS_LIST   13ULL

#define SYS_EXEC      20ULL

#define SYSCALL_VECTOR 0x80

// User address window for pointer validation:
// shell code/stack + program slot code/stack (see vmm.h layout).
#define USER_AREA_BASE 0x400000ULL
#define USER_AREA_END  0x700000ULL

// Max bytes per syscall buffer op (fits kernel heap temp allocs).
#define SYSCALL_MAX_BUF 2048ULL
#define SYSCALL_MAX_PATH 256ULL

// Must match push order in isr_syscall (src/arch/x86_64/cpu/isr.s):
// push rax,rdi,rsi,rcx,rdx,rbx,rbp,r8..r15, then CPU frame follows.
typedef struct syscall_frame {
  uint64_t r15, r14, r13, r12, r11, r10, r9, r8;
  uint64_t rbp, rbx, rdx, rcx, rsi, rdi, rax;
  uint64_t rip, cs, rflags, rsp, ss;
} syscall_frame_t;

void syscall_handler(syscall_frame_t *f);
bool syscall_init(void);

#endif // NYV_SYSCALL_H
