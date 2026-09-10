section .data
  CONTEXT_T_SIZE equ 152
  UINT64_T_SIZE equ 8

section .bss
  ctx_ptr resq 1
  RSP_REG resq 1

section .text
  global save_context
  
  extern kmalloc

save_context:
  mov [RSP_REG], rsp

  push r15
  push r14 
  push r13
  push r12
  push r11
  push r10
  push r9
  push r8
  push rbp
  push rbx
  push rdx
  push rcx
  push rsi
  push rdi
  push rax

  mov rax, CONTEXT_T_SIZE
  call kmalloc

  cmp rax, 0
  je .err
 
  mov [ctx_ptr], rax
  mov rdi, [ctx_ptr] ; dereference
  
  mov rdx, [RSP_REG]

  pop rax
  mov [rdi], rax
  
  mov rax, rdi
  
  pop rdi
  mov [rax + UINT64_T_SIZE * 1], rdi

  pop rsi
  mov [rax + UINT64_T_SIZE * 2], rsi

  pop rcx
  mov [rax + UINT64_T_SIZE * 3], rcx

  pop rdx
  mov [rax + UINT64_T_SIZE * 4], rdx

  pop rbx
  mov [rax + UINT64_T_SIZE * 5], rbx
  
  pop rbp
  mov [rax + UINT64_T_SIZE * 6], rbp
  
  mov rdi, [RSP_REG]
  mov [rax + UINT64_T_SIZE * 7], rdi

  pop r8
  mov [rax + UINT64_T_SIZE * 8], r8

  pop r9
  mov [rax + UINT64_T_SIZE * 9], r9  

  pop r10 
  mov [rax + UINT64_T_SIZE * 10], r10  

  pop r11
  mov [rax + UINT64_T_SIZE * 11], r11  

  pop r12
  mov [rax + UINT64_T_SIZE * 12], r12  

  pop r13
  mov [rax + UINT64_T_SIZE * 13], r13

  pop r14
  mov [rax + UINT64_T_SIZE * 14], r14  

  pop r15
  mov [rax + UINT64_T_SIZE * 15], r15
  
  mov rdx, [RSP_REG]
  mov [rax + UINT64_T_SIZE * 16], rdx

  pushfq
  pop rdx

  mov [rax + UINT64_T_SIZE * 17], rdx

  mov rdx, cr3
  mov [rax + UINT64_T_SIZE * 18], rdx
  
  ret

  .err:
    add rsp, 15 * 8
    xor eax, eax
    ret
