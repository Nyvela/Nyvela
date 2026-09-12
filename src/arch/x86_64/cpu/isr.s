section .text
  global isr_de
  extern isr_de_handler

  global isr_pf
  extern isr_pf_handler

  global isr_lapic_timer
  extern isr_lapic_timer_handler
  extern klapic_eoi

isr_de:
  call isr_de_handler
  iretq

isr_pf:
  call isr_pf_handler
  add rsp, 8
  iretq

isr_lapic_timer:
  call isr_lapic_timer_handler
  
  call klapic_eoi 

  iretq
