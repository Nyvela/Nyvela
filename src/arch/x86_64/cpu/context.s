section .data
  CONTEXT_T_SIZE equ 168
  UINT64_T_SIZE equ 8

section .bss
  ctx_ptr resq 1
  rsp_reg resq 1

section .text
  global save_context
  global switch_context

  extern kmalloc

; Stack currently looks like this
; [  ss      ]
; [  rflags  ]
; [  old rsp ]
; [  cs      ]
; [  rip     ]
; [  rsp     ]
save_context:
  mov [rsp_reg], rsp

  ; [rsp_reg + 8] - rip
  ; [rsp_reg + 16] - cs
  ; [rsp_reg + 24] - rflags
  
  push rax
  push rdi
  push rsi
  push rcx
  push rdx
  push rbx
  push rbp
  
  ; rsp is already saved
  
  push r8
  push r9
  push r10
  push r11
  push r12
  push r13
  push r14
  push r15

  ; rip, rflags and cs are already saved
  
  mov rdi, 168
  call kmalloc

  test rax, rax
  jz .err

  mov rdi, cr3
  mov [rax + 160], rdi

  mov rdx, [rsp_reg]

  mov [rax + 152], 0x10 ; ss
   
  mov rdi, [rdx + 16] ; cs
  mov [rax + 144], rdi

  mov rdi, [rdx + 24] ; rflags
  mov [rax + 136], rdi  
 
  mov rdi, [rdx + 8] ; rip
  mov [rax + 128], rdi

  pop r15
  mov [rax + 120], r15

  pop r14
  mov [rax + 112], r14

  pop r13
  mov [rax + 104], r13

  pop r12
  mov [rax + 96], r12

  pop r11
  mov [rax + 88], r11

  pop r10
  mov [rax + 80], r10

  pop r9
  mov [rax + 72], r9

  pop r8
  mov [rax + 64], r8

  mov rdi, [rsp_reg]
  add rdi, 32
  mov [rax + 56], rdi
   
  pop rbp 
  mov [rax + 48], rbp

  pop rbx
  mov [rax + 40], rbx

  pop rdx
  mov [rax + 32], rdx
  
  pop rcx
  mov [rax + 24], rcx

  pop rsi
  mov [rax + 16], rsi

  pop rdi
  mov [rax + 8], rdi
  
  pop rdi ; move rax's value to rdi
  mov [rax], rdi

  mov rsp, [rsp_reg]
  ret
  
  .err:
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

    mov rsp, [rsp_reg]
    xor rax, rax
    ret

; rdi = context_t*
switch_context: 
  mov rax, [rdi + 160]
  mov cr3, rax
  
  mov rsp, [rdi + 56]
  
  push qword [rdi + 152] ; ss
  push qword [rdi + 56] ; rsp 
  push qword [rdi + 136] ; rflags
  push qword [rdi + 144] ; cs
  push qword [rdi + 128] ; rip
  
  mov r15, [rdi + 120]
  mov r14, [rdi + 112]
  mov r13, [rdi + 104]

  mov r12, [rdi + 96]
  mov r11, [rdi + 88]
  mov r10, [rdi + 80]
  mov r9, [rdi + 72]
  mov r8, [rdi + 64]
  mov rbp, [rdi + 48]
  mov rbx, [rdi + 40]
  mov rdx, [rdi + 32]
  mov rcx, [rdi + 24]
  mov rsi, [rdi + 16] 
  mov rax, [rdi]
  mov rdi, [rdi + 8]

  iretq
