BITS 16
[org 0x8000]

stage_1:
  cli

  mov ax, 0x0800
  mov es, ax
  mov ss, ax
  
  xor ax, ax
  mov ds, ax
  
  sti

  mov si, success_msg
  call print
  
  mov si, gdt_msg
  call print

  cli

  .hlt:
    hlt
    jmp .hlt

%include "print.s"  

success_msg: db "[ SUCC ] Entered Stage 1.", 0
gdt_msg: db "[ INFO ] Setting up GDT...", 0
