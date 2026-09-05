BITS 16
[org 0x7C00]

global _start

_start:
  cli

  mov [0x7B00], dl

  mov ax, 0x07C0
  mov ss, ax

  xor ax, ax
  mov ds, ax
  mov dword [0xC700], eax

  sti

  mov ax, 0x0003 ; Set video mode = 80x25
  int 0x10

  mov si, boot_msg
  mov ah, 0x0F
  call printinit

  mov ah, 0x42 ; Read from disk
  mov si, DAP
  mov dl, [0x7B00] ; load boot drive number

  int 0x13
  jc stage_1_disk_error

  mov si, success_msg
  mov ah, 0x0F
  call printsucc

  mov si, far_jump_msg
  mov ah, 0x0F
  call printinfo

  jmp 0x0800:0x0000 ; stage 1

  stage_1_disk_error:
    mov si, stage_1_err_msg
    mov ah, 0x0F
    call printferr
    jmp halt

  halt:
    hlt
    jmp halt

DAP: ; Disk Address Packet, required for BIOS's INT13h extensions
  db 0x10 ; Size of packet
  db 0x0 ; Always 0 for some reason
  dw 0x0003 ; 2 sectors to read, 1536 bytes size cap for stage 1
  
  ; Physical address for where to load data, long jump here
  dw 0x0000 ; offset
  dw 0x0800 ; segment
  
  dq 0x01 ; LBA

%include "print.s" ; print, println, printnl

boot_msg: db "Loading Stage 1 to memory...", 0
stage_1_err_msg: db "Failed to load Stage 1 to memory.", 0
success_msg: db "Loaded Stage 1 successfully to memory.", 0
far_jump_msg: db "Performing far jump to Stage 1...", 0

times 510 - ($ - $$) db 0
dw 0xAA55
