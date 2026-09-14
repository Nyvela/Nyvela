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

; Debug fault dumper entries. fault_dump(vec, err, has_err, frame*)
; never returns (halts), so no register saving is needed.
isr_ud: ; #UD has no CPU error code
  mov rdi, 6
  xor esi, esi
  xor edx, edx
  mov rcx, rsp
  call fault_dump
  cli
  hlt
  jmp $-2

isr_df: ; #DF pushes an error code (always 0)
  pop rsi ; err
  mov rdi, 8
  mov edx, 1
  mov rcx, rsp
  call fault_dump
  cli
  hlt
  jmp $-2

isr_lapic_timer:
  call save_context ; rax is now context_t*
  mov rdi, rax
  jmp isr_lapic_timer_handler

isr_pit:
  ; isr_pit_handler() is C and may clobber caller-saved regs.
  ; Save them so the 1000Hz PIT doesn't corrupt interrupted code.
  push rax
  push rcx
  push rdx
  push rsi
  push rdi
  push r8
  push r9
  push r10
  push r11
  call isr_pit_handler
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

; PS/2 keyboard, IRQ1 (PIC vector 0x21, ring0 only).
; Same save discipline as isr_pit: the C handler may clobber caller-saved regs.
isr_kbd:
  push rax
  push rcx
  push rdx
  push rsi
  push rdi
  push r8
  push r9
  push r10
  push r11
  call kbd_irq_handler
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

; int 0x80 syscall entry (ring3 -> ring0).
; CPU pushes ss/rsp/rflags/cs/rip, loads rsp0 from TSS.
; Frame layout must match syscall_frame_t in syscall.h:
;   r15..rax (15 pushes) then rip/cs/rflags/rsp/ss from CPU.
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
