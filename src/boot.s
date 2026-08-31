[org 0x7C00]

global _start

_start:
  cli

  mov ax, 0x7C00
  mov ds, ax
  mov ss, ax
  mov es, ax

  sti

  jmp $

times 510 - ($ - $$) db 0
dw 0xAA55
