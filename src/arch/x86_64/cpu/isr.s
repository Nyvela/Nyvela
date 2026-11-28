section .bss
  syscall_user_rsp resq 1

section .text
  global isr_lapic_timer
  extern isr_lapic_timer_handler

  extern save_context

  global isr_pit
  extern isr_pit_handler

  global isr_kbd
  extern kbd_irq_handler

  extern fault_dump
  extern lumen_user_fault

  global isr_syscall
  global syscall_return
  extern syscall_handler
  global isr_elf_syscall
  extern lumen_elf_syscall
  extern tss

%macro exception 3
  global %1
%1:
%if %3
  pop rsi
%else
  xor esi, esi
%endif
  mov edi, %2
  mov edx, %3
  mov rcx, rsp
  jmp fault
%endmacro

exception isr_de, 0, 0
exception isr_db, 1, 0
exception isr_bp, 3, 0
exception isr_of, 4, 0
exception isr_br, 5, 0
exception isr_ud, 6, 0
exception isr_nm, 7, 0
exception isr_df, 8, 1
exception isr_ts, 10, 1
exception isr_np, 11, 1
exception isr_ss, 12, 1
exception isr_gp, 13, 1
exception isr_pf, 14, 1
exception isr_mf, 16, 0
exception isr_ac, 17, 1
exception isr_xm, 19, 0

isr_lapic_timer:
  call save_context ; rax is now context_t*
  cld
  and rsp, -16
  mov rdi, rax
  call isr_lapic_timer_handler
  ud2

; These IRQs resume the interrupted instruction stream directly. Protect
; the registers that a SysV C handler may clobber across the call.
%macro irq_handler 2
%1:
  push rax
  push rcx
  push rdx
  push rsi
  push rdi
  push r8
  push r9
  push r10
  push r11
  push r12
  mov r12, rsp
  and rsp, -16
  cld
  call %2
  mov rsp, r12
  pop r12
  pop r11
  pop r10
  pop r9
  pop r8
  pop rdi
  pop rsi
  pop rdx
  pop rcx
  pop rax
  iretq
%endmacro

irq_handler isr_pit, isr_pit_handler
irq_handler isr_kbd, kbd_irq_handler

%macro save_syscall_registers 0
  push rax
  push rdi
  push rsi
  push rcx
  push rdx
  push rbx
  push rbp
  push r8
  push r9
  push r10
  push r11
  push r12
  push r13
  push r14
  push r15
%endmacro

isr_syscall:
  save_syscall_registers
  cld
  mov rdi, rsp
  call syscall_handler
  jmp syscall_return

; SYSCALL leaves RSP alone and saves RIP/RFLAGS in RCX/R11. Only the
; boot CPU runs userspace, so the entry scratch slot is CPU-local in practice.
isr_elf_syscall:
  mov [rel syscall_user_rsp], rsp
  mov rsp, [rel tss + 4]
  push rax
  mov ax, 0x10
  mov ss, ax
  pop rax
  push 0x23
  push qword [rel syscall_user_rsp]
  and r11, 0x378fd7 ; clear user IOPL and NT before the IRETQ frame
  push r11
  push 0x1b
  push rcx
  save_syscall_registers
  mov rdi, rsp
  call lumen_elf_syscall

syscall_return:
  pop r15
  pop r14
  pop r13
  pop r12
  pop r11
  pop r10
  pop r9
  pop r8
  pop rbp
  pop rbx
  pop rdx
  pop rcx
  pop rsi
  pop rdi
  pop rax

  iretq

fault:
  cld
  and rsp, -16
  mov r10, [rcx + 8]
  and r10, 0x03

  cmp r10, 3 ; CPL3
  je .recover

  call fault_dump
  
  cli
  hlt

  .recover:
    call lumen_user_fault
    ud2
