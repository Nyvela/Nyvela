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

  extern kprintferr

isr_de:
  mov rdi, de_msg
  mov rsi, 0x0F
  call kprintferr

  jmp halt

isr_pf:
  mov rdi, pf_msg
  mov rsi, 0x0F
  call kprintferr

  add rsp, 8
  jmp halt

isr_gp:
  mov rdi, gp_msg
  mov rsi, 0x0F
  call kprintferr
  
  add rsp, 8
  jmp halt
  
halt:
  hlt
  jmp halt

isr_lapic_timer:
  call save_context ; rax is now context_t*
  mov rdi, rax
  jmp isr_lapic_timer_handler
