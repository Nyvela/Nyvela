section .text
  global isr_de
  extern isr_de_handler

  global isr_pf
  extern isr_pf_handler

isr_de:
  call isr_de_handler
  iretq

isr_pf:
  call isr_pf_handler
  add rsp, 8
  iretq
  
