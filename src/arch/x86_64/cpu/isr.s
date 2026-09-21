section .data
  de_msg db "Division Error.", 0
  pf_msg db "Page Fault.", 0
  gp_msg db "General Protection.", 0

section .text
  global isr_de
  global isr_pf
  global isr_gp

  global isr_lapic_timer
  extern isr_lapic_timer_handler

  extern save_context

  global isr_pit
  extern isr_pit_handler

  global isr_kbd
  extern kbd_irq_handler

  global isr_ud
  global isr_df
  extern fault_dump

  global isr_syscall
  extern syscall_handler

  extern kprintferr

isr_de:
  mov rdi, 0
  xor rsi, rsi
  xor rdx, rdx
  mov rcx, rsp 

  call fault_dump

  cli
  hlt

isr_pf:
  pop rsi ; err
  mov rdi, 8
  mov edx, 1
  mov rcx, rsp
  call fault_dump
  
  cli
  hlt

isr_gp:
  pop rsi ; err
  mov rdi, 0x0D
  mov edx, 1
  mov rcx, rsp
  call fault_dump
  
  cli
  hlt
  
halt:
  hlt
  jmp halt

isr_ud:
  mov rdi, 6
  xor esi, esi
  xor edx, edx
  mov rcx, rsp
  call fault_dump

  cli
  hlt  

isr_df:
  pop rsi ; err
  mov rdi, 8
  mov edx, 1
  mov rcx, rsp
  call fault_dump
  
  cli
  hlt

isr_lapic_timer:
  call save_context ; rax is now context_t*
  mov rdi, rax
  jmp isr_lapic_timer_handler

isr_pit:
  call isr_pit_handler
  iretq

isr_kbd:
  call kbd_irq_handler
  iretq

isr_syscall:
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

  mov rdi, rsp
  call syscall_handler

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
