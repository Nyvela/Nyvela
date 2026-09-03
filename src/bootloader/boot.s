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
  mov si, DAP_stage_1
  mov dl, [bootdrive] ; load boot drive number

  int 0x13
  jc stage_1_disk_error

  mov ah, 0x42 ; Read from disk
  mov si, DAP_stage_2
  mov dl, [bootdrive] ; load boot drive number

  int 0x13
  jc stage_2_disk_error

  mov si, success_msg
  call print

  mov si, far_jump_msg
  call print

  jmp 0x0800:0x0000 ; stage 1

  stage_1_disk_error:
    mov si, stage_1_err_msg
    call print
    jmp halt

  stage_2_disk_error:
    mov si, stage_2_err_msg
    call print
    jmp halt

  halt:
    hlt
    jmp halt

bootdrive: db 0

DAP_stage_1: ; Disk Address Packet, required for BIOS's INT13h extensions
  db 0x10 ; Size of packet
  db 0x0 ; Always 0 for some reason
  dw 0x0002 ; 2 sectors to read, 1024 bytes size cap for stage 1
  
  ; Physical address for where to load data, long jump here
  dw 0x0000 ; offset
  dw 0x0800 ; segment
  
  dq 0x01 ; LBA

DAP_stage_2: ; Disk Address Packet, required for BIOS's INT13h extensions
  db 0x10 ; Size of packet
  db 0x0 ; Always 0 for some reason
  dw 0x0001 ; 1 sector to read, 512 bytes size cap for stage 2
  
  ; Physical address for where to load data, long jump here
  dw 0x0000 ; offset
  dw 0x0840 ; segment
  
  dq 0x03 ; LBA

%include "print.s"

init_msg: db "[ INIT ] Preparing to load Stage 1 and Stage 2 to memory.", 0
stage_1_err_msg: db "[ FAIL ] Failed to load Stage 1 to memory.", 0
stage_2_err_msg: db "[ FAIL ] Failed to load Stage 2 to memory.", 0
success_msg: db "[ SUCC ] Loaded Stage 1 and Stage 2 successfully to memory.", 0
far_jump_msg: db "[ INFO ] Performing far jump to Stage 1...", 0

times 510 - ($ - $$) db 0
dw 0xAA55
