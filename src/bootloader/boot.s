BITS 16
[org 0x7C00]

global _start

_start:
  cli

  mov [bootdrive], dl

  mov ax, 0x07C0
  mov ss, ax
  mov es, ax

  xor ax, ax
  mov ds, ax

  sti

  mov ah, 0x00 ; set video mode
  mov al, 0x03 ; 80x25
  int 0x10

  mov si, init_msg
  call print

  mov ah, 0x42 ; Read from disk
  mov si, DAP
  mov dl, [bootdrive] ; load boot drive number

  int 0x13
  jc disk_error

  mov si, success_msg
  call print

  mov si, far_jump_msg
  call print

  jmp 0x0800:0x0000

  disk_error:
    mov si, disk_error_msg
    call print
    jmp .hlt

  .hlt:
    hlt
    jmp .hlt

bootdrive: db 0

DAP: ; Disk Address Packet, required for BIOS's INT13h extensions
  db 0x10 ; Size of packet
  db 0x0 ; Always 0 for some reason
  dw 0x0001 ; 1 sector to read, 512 bytes size cap for stages 1 and 2
  
  ; Physical address for where to load data, long jump here
  dw 0x0000 ; offset
  dw 0x0800 ; segment
  
  dq 0x01 ; LBA

%include "print.s"

init_msg: db "[ INIT ] Preparing to load Stage 1 and Stage 2 to memory.", 0
disk_error_msg: db "[ FAIL ] Failed to load Stage 1 and Stage 2 to memory.", 0
success_msg: db "[ SUCC ] Loaded Stage 1 and Stage 2 successfully to memory.", 0
far_jump_msg: db "[ INFO ] Performing far jump to Stage 1...", 0

times 510 - ($ - $$) db 0
dw 0xAA55
