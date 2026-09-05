; print32.s is available here

isr_de:
  cli
  hlt
  jmp isr_de

isr_gp:
  cli
  hlt
  jmp isr_gp

isr_de_err: db "Division Error.", 0
isr_gp_err: db "General Protection Error.", 0
